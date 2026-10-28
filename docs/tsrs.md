# ErrorScript and tsrs

Examined upstream commit `dc4ea354270c729c3b28b48d9ef008ec8ac2b390`
(tsrs 0.20.0, TypeScript 7.1.0-beta reference). The Cargo dependencies pin that
commit; they do not depend on the local checkout. It requires Rust 1.99.

## 1. From tsrs CLI to types

Crate source paths below are under `crates/` in the tsrs repository.

| Step      | Entry point                                                                         | Crates and data passed onward                                                                                                                                                                                                                  |
| --------- | ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI       | `tsrs_cli/src/main.rs::main`                                                        | `Vec<String>`; `tsrs_execute::sys::OsSys`. Normal checking runs on a thread with a 512 MiB stack. `--lsp` and `--api` take separate routes.                                                                                                    |
| Dispatch  | `tsrs_execute/src/execute.rs::command_line`, `command_line_with_testing`            | `tsrs_execute::tsc::System`, filesystem `Arc<dyn tsrs_vfs::FS>`, `CommandLineResult`. Selects ordinary compilation, build or watch.                                                                                                            |
| Options   | `tsrs_tsoptions::parse_command_line`; `execute.rs::tsc_compilation`                 | `ParsedCommandLine`, `ParsedOptions`, `P<tsrs_core::CompilerOptions>`, root filenames and project references. CLI finds/validates `-p` or discovers `tsconfig.json`.                                                                           |
| Config    | `tsrs_tsoptions::get_parsed_command_line_of_config_file` in `tsconfigparsing.rs`    | `ParseConfigHost`, `TsConfigSourceFile`, `ExtendedConfigCache`, `ParsedCommandLine`, `P<tsrs_ast::Diagnostic>`. Handles JSON config, extends, files/include/exclude and reference metadata. This is the config authority.                      |
| Host      | `tsrs_compiler::new_cached_fs_compiler_host` / `new_compiler_host`                  | `Arc<dyn CompilerHost>`, `tsrs_vfs` OS/cache/bundled-library filesystem; resolution and parsing are host operations.                                                                                                                           |
| Program   | `tsrs_compiler::new_program(ProgramOptions)`                                        | `Program`, `ProgramConfig`, `FileLoader`, `filesparser::ParseTask`, `ProcessedFiles`, `SourceFileMetaData`, project-reference parser/mapper. `tsrs_module` supplies `ResolvedModule` and resolution-mode caches.                               |
| Parse     | Compiler host -> `tsrs_parser::parse_source_file_owned` or embedded-file equivalent | `SourceFileParseOptions`, `ScriptKind`, `P<SourceFile>`, `P<Node>`, `NodeList`, `Kind`; scanner in `tsrs_scanner`. This parser is distinct from Oxc's parser. Positions are UTF-8 byte offsets (`TextPos`), with a synthetic-position variant. |
| Bind      | `Program::bind_source_files`, `tsrs_binder`                                         | Lexical/module `Symbol` tables, declaration links, `FlowNode` control-flow graph. Parsed and bound ASTs are shared read-only between checkers.                                                                                                 |
| Check     | `tsrs_execute/src/tsc/emit.rs` -> `Program::get_semantic_diagnostics`               | `Context`, `CheckerPoolImpl`, `CheckerHandle`, `tsrs_checker::Checker`; per-file diagnostics plus global/config/syntax diagnostics. The checker lazily computes and caches types.                                                              |
| Query     | `Program::get_type_checker` or `get_type_checker_for_file`                          | A live `CheckerHandle`; `get_type_at_location(P<Node>) -> P<Type>`, `get_resolved_signature_exported -> P<Signature>`, symbol/property/constraint/assignability APIs. A diagnostics vector is **not** the type database.                       |
| Interpret | `tsrs_checker/src/types.rs`, `exports.rs`, `printer.rs`                             | `TypeFlags`, `ObjectFlags`, `Type`, unions/intersections, aliases, instantiated references, `Signature`, type arguments; `type_to_string_exported` renders a type. Rendering must never substitute for type identity.                          |

The native API demonstrates these same queries in
`tsrs_api/src/checker/handlers_types.rs`. ErrorScript can call the Rust APIs
directly; it does not need the CLI, Node bridge, JSON protocol, or LSP hovers.

### Ownership is part of the integration

`tsrs_core::P<T>` is a copyable identity pointer, not an owning smart pointer.
Checkers have their own mutable caches and type identities. Sending one checker's
types to another checker, or retaining them after its region is freed, is invalid.
`Program` construction returns a nominally static reference; reclamation requires
the `free_program`/`free_unshared_program` contract. `ProgramRegions`, checker
regions, file regions and snapshot references have different lifetimes.

The adapter uses one checker per program, region-backed allocation, scoped
handles, and owned output. It disables neither checking nor inference for JS;
`allowJs` includes runtime files, while the user's `checkJs` controls diagnostics.
The CLI's leaf-AST disposal and checker-recycling optimizations must stay off
when subsequent analysis needs those ASTs or types.

### What TypeScript does not provide

`tsrs_checker/src/checker_02.rs::check_throw_statement` checks the expression. It
does not attach a thrown-type effect to the enclosing signature. `flow.rs` has
flow-sensitive reference typing, not an interprocedural exception analysis.

Full TypeScript information still includes `any`, `unknown`, assertions,
structural function types and declaration-only dependencies. A selected overload
does not prove the runtime function identity. `Promise<T>` describes fulfillment,
not rejection. JS without parameter annotations generally has `any` parameters;
enabling `checkJs` is not whole-program caller-to-callee inference.

## 2. Existing ErrorScript

| Area          | Implementation before this change                    | Consequence                                                                                                                                                                                                             |
| ------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Project       | `project/{mod,file,parse}.rs`, `resolver.rs`         | Oxc config/resolution and a separate file-pattern walker; runtime and declaration dependencies are both loaded.                                                                                                         |
| Syntax        | `module/mod.rs`                                      | `EscModule` owns an Oxc allocator, `Program` and `Semantic` through `self_cell`. Workers may parse another copy.                                                                                                        |
| Linking       | `checker/call_graph.rs`                              | Custom abstract values for bindings, aliases, ESM/CJS exports, namespaces, constructors, callbacks, annotations and inheritance. `EscCallGraph` uses petgraph and Tarjan SCCs.                                          |
| Values        | `checker/errors/{mod,types,expression,arguments}.rs` | A second, partial type system: builtin/declaration identities, manual annotation expansion, property lookup, value propagation, argument propagation and narrowing. Generic/conditional/mapped types exceed this model. |
| Effects       | `checker/errors/{statement,expression,queue}.rs`     | Completion-sensitive may-throw analysis, SCC fixed points, per-function summaries, synchronous vs deferred failures, catch/finally, defaults and runtime operations. These semantics are not supplied by tsrs.          |
| Host behavior | `globals/{mod,data}.rs`                              | Builtin effects, argument coercions and global mutation. TypeScript's standard library does not specify these exceptions.                                                                                               |
| Editor        | `editor.rs`, `pkgs/lsp/src/lib.rs`                   | Persistent sources, unsaved overlays, invalidation, call reports and catch-cast diagnostics; LSP publishes owned snapshots.                                                                                             |

The valuable distinction is between **value typing** and **runtime effects**.
Reusing the effect model does not justify retaining a second annotation resolver.
Conversely, replacing that resolver does not make TypeScript an exception checker.

## 3. Clean-sheet design

The preferred end state is one tsrs-owned project snapshot and syntax graph:

1. Use `tsrs_project` for configured/inferred projects and editor overlays;
   `tsrs_compiler::Program` is the batch analysis boundary. Preserve project
   reference compiler options rather than treating a monorepo as one compilation.
2. Use native `Node`/`Symbol`/`Signature` identities throughout. Query actual
   expression types, contextual signatures and instantiated property types.
   Do not parse printed types or reimplement type-annotation syntax.
3. Lower executable native ASTs to a compact effect IR. Keep completion kind,
   source range, enclosing function, call candidates, evaluated operands and
   references to native type facts. Build this IR once per changed source.
4. Solve an **effect** fixed point, not another general-purpose type checker.
   A summary needs synchronous throws, rejection/iterator effects, normal
   completion, callback effects, escaped deferred failures and captured writes.
   SCCs solve recursive bodies without merging distinct function summaries.
5. Represent generic effects symbolically: for `raise<T>(x: T) { throw x }`, an
   effect is the parameter's type variable, instantiated with the selected
   signature at a call. Higher-order effects need callback variables as well.
   Merely querying the generic body gives `T`, not each caller's instantiation.
6. Maintain a separate runtime module graph. TS resolves a package to its `.d.ts`;
   ErrorScript may need its JS entry and implementation. Track ESM/CJS conditions,
   type-only imports and declaration/runtime correspondence explicitly. A same-name
   file or exported type is not proof of implementation correspondence.
7. For JS `any`, propagate a finite abstract runtime domain from call arguments,
   assignments and effect summaries. Preserve an unknown alternative for exported
   functions and escaping values. Never reinterpret `any` as no failures.
8. Cache by program/snapshot identity plus relevant source/config/dependency
   versions. Type-only dependencies, global declarations, lib changes and package
   resolution changes also invalidate effects. Share native binding work; avoid
   parsing a source once per worker. Measure before choosing parallel checkers.

No fork is necessary to start this design: the public native queries already
cover much more than ErrorScript's handwritten value typing. The hardest parts
are effect polymorphism, runtime/declaration correspondence and sound invalidation,
not retrieving a type's display name.

## 4. Implementation status in this change

`pkgs/crate/src/typescript.rs` is a native, pinned-Git compiler adapter. It loads
TypeScript configs and bundled libraries, overlays parsed/editor source text,
checks programs, and extracts owned value/property/signature facts. Native
diagnostics and exact-range rendered types are available through
`EscCallGraph::typescript_diagnostics` and `type_at`.

The manual annotation/alias/interface-property resolver has been replaced with
native queries. Distinct generic instantiations have distinct snapshot-local
identities. Primitive widening is an explicit operation of the effect domain;
the un-widened native rendering remains queryable. JSDoc parameter types and
generic call results can supply information that was previously unknown. Native
signature declarations add call-graph candidates without claiming that virtual
dispatch is uniquely resolved. Runtime uncertainty survives native type queries.
Generic class instances retain their runtime class ancestry for `instanceof`.
Getter reads retain unknown effects, and typed promises distinguish fulfillment
types from their unspecified rejection effects.

Editor rebuilds provide all current overlays to the compiler, including ambient
declarations with no runtime import edge. Disk reads (including extended configs)
are fingerprinted for invalidation. All effects are rechecked on a changed
snapshot; unchanged syntax arenas and equal report allocations are retained.
This favors correctness over the old component-only checking optimization until
native project snapshots can provide complete dependency invalidation.

**This is a type-backend migration, not the completed clean-sheet rewrite.**
The Oxc runtime AST, runtime linker, effect evaluator and worker scheduler still
exist. The adapter groups inputs by nearest `tsconfig.json`, rather than using the
full `tsrs_project` configured-project lifecycle. Its owned facts are a finite
projection, not a persistent live checker. Generic effect substitution, complete
native AST lowering and native incremental editor projects remain implementation
work. The architecture above describes the target, not features already shipped.

## 5. If we can change tsrs

### Picture A: ErrorScript is a tsrs fork

ErrorScript becomes an optional compiler mode, built into tsrs's project system,
checker and language service. Ordinary TypeScript semantics stay available when
the mode is off.

- Extend signature metadata with an `EffectSignature` (throw/rejection/yield and
  callback effects), keyed separately from ordinary return types. Carry effect
  variables through overload selection, instantiation, contextual typing and
  generic inference. `raise<RangeError>` can then expose `RangeError` directly.
- Add an effect pass over binder control flow and structured AST completions.
  Extend it with exceptional edges, catch entry types and finally completion
  replacement. Existing flow nodes cannot simply be assumed to encode all these
  semantics. This supplies typed catch bindings without synthetic source casts.
- Track promise rejection effects and handler transformations separately from
  the structural `Promise<T>` value type. Await releases the rejection channel;
  `.catch`, `.then` and `.finally` transform it.
- Make effect constraints part of higher-order checking: a callback that can
  throw affects its invoker according to the invoker's effect signature. This is
  substantially more precise than joining all observed callbacks globally.
- Integrate runtime-source resolution, implementation pairing and builtin effect
  metadata into project loading. Optional JS inference can supply candidate value
  types to the checker while leaving normal TS-mode inference unchanged.
- Add native diagnostics, hover/inlay information, catch code actions and optional
  effect declaration emission. Extend incremental build fingerprints so changing
  a callee's effects invalidates callers even if its TS return type is unchanged.

This gives maximum precision and a single compiler/editor pipeline. It also
requires owning language semantics: syntax or declaration metadata for effects,
effect subtyping, variance, recursive inference, contextual callback effects,
error recovery, serialization and compatibility tests. It is not just adding a
`throws: Type` field. Fork maintenance touches the checker, binder, compiler,
project system, declaration emitter, incremental engine and LS.

### Picture B: Thin maintained tsrs fork with integration hooks

Keep ErrorScript as an independent effect crate. Upstream TypeScript inference
remains unchanged. Prefer narrow semantic APIs over exposing arbitrary mutable
checker fields.

| Hook/API                                                                                                                         | Concrete improvement                                                                                                                                                                                                         |
| -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scoped program/checker session with owned snapshot handles                                                                       | Makes arena/checker lifetimes explicit in Rust and prevents accidental cross-checker type use or use after region reclamation. Enables persistent editor queries without copying every type.                                 |
| Resolved-call record: selected declaration, overload/implementation correspondence, type arguments and a scoped signature mapper | Instantiates ErrorScript's generic effect variables using the same substitutions as TS, rather than reverse-engineering a printed signature.                                                                                 |
| Public `instantiate_effect_type(type, call_context)`-style operation                                                             | Lets an external effect pass substitute parameter/return/callback type references while keeping internal mapper representation private.                                                                                      |
| Read-only flow query plus supported narrowing of a supplied seed type                                                            | Lets ErrorScript seed a catch variable with the inferred throw union and ask TS to narrow it through `instanceof`, discriminants and predicates. Current `get_type_at_location` only sees the checker's original catch type. |
| Structured exceptional-completion/flow visitor                                                                                   | Avoids a second control-flow implementation; exposes catch/finally boundaries and reachable normal/abrupt paths without changing ordinary TS diagnostics.                                                                    |
| Project snapshot dependency/invalidation events                                                                                  | Exposes type-only, ambient, configuration, resolution and project-reference changes; invalidates effect summaries consistently with compiler state.                                                                          |
| Side-table attachment and stable snapshot-local identity APIs                                                                    | Associates effects with native nodes/signatures without modifying or duplicating ASTs, and makes cache disposal explicit.                                                                                                    |
| Runtime resolution hook alongside normal declaration resolution                                                                  | Retains both implementation and declaration graphs and reports when no reliable pairing is known.                                                                                                                            |
| Explicit external JS candidate-type provider                                                                                     | Allows bounded caller-driven JS inference as an opt-in source for queries. It must retain provenance and unknown alternatives; silently replacing TS `any` globally would alter TS behavior.                                 |

The thin fork is the preferable long-term boundary if ErrorScript stays a separate
tool. Start with lifetime-safe sessions, call-instantiation context and project
invalidation; these have clear contracts and do not require changing TypeScript
syntax. Catch-flow seeding and higher-order effect inference are deeper extensions.

Neither picture can infer the implementation of an unavailable `.d.ts` dependency,
make arbitrary `eval`/proxies decidable, or make an unchecked assertion true at
runtime. Those cases need effect contracts, trusted host models or explicit
unknown effects. A fork improves integration and precision; it does not remove
these information limits.
