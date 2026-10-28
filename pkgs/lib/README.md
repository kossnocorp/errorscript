# Built-in library test inventory

`errorscript-lib` is a Rust library and CLI that parses TypeScript declarations with
Oxc and synchronizes editable Vitest files. The tests will be the behavioral source
of truth for future built-in `Returns` annotations. New API suites contain only:

```ts
it("TODO", () => {
  // TODO
});
```

## Generate

From the repository root:

```sh
pnpm install
cargo run -p errorscript-lib
cargo run -p errorscript-lib -- --check
```

Or use `pnpm --filter @errorscript/lib generate` / `check`.

Defaults:

| Input                                                                                              | Output                                |
| -------------------------------------------------------------------------------------------------- | ------------------------------------- |
| `subs/errs-vscode/extensions/typescript-language-features/dist/development/server/libs/lib.*.d.ts` | `pkgs/lib/tests/ts/*.test.ts`         |
| `pkgs/lib/node_modules/@types/node/**/*.d.ts`                                                      | `pkgs/lib/tests/node/24/**/*.test.ts` |

Node **24** is the initial runtime line. Its declarations are pinned to
`@types/node@24.19.2`; the generator verifies the installed package's major version.
The `ts5.6`, `ts5.7`, etc. directories are alternative TypeScript compiler compatibility
trees and are excluded, rather than mistaken for Node runtime versions.

Custom inputs and additional runtime lines:

```sh
cargo run -p errorscript-lib -- \
  --ts-libs /path/to/typescript/lib \
  --node-types /path/to/node24-types \
  --node-version 24 \
  --out pkgs/lib/tests
```

`--out` is the parent of `ts/` and `node/<major>/`. Explicit relative paths are
relative to the current directory. Defaults work from any directory. Running for
another Node major leaves the existing version directories alone. The CLI uses
local inputs; dependency installation and version updates are explicit.

## Describe paths

TypeScript test files have one outer library suite, named from the declaration
filename: `ES2019.String`, `ES2020.BigInt`, `DOM`, `WebWorker`, etc. API suites are
nested inside it. Existing flat files are migrated while retaining their tests;
imports and exports stay at module scope.

- Functions: `parseInt` → tests.
- Interfaces/object declarations: `TransitionEvent` → `new`, `initEvent`, etc.
- Class constructors: `Example` → `new`.
- Class methods: `Example` → `run`; static methods: `Example` → `static run`.
- Explicit accessors: `Example` → `get value` / `set value`.
- Namespaces and global blocks retain their nesting.
- Node modules use canonical `node:` names, e.g.
  `node:console` → `global` → `Console` → `log`.
- Computed keys retain their expression, e.g. `[Symbol.iterator]`.
- Callable properties, function/constructor types, and locally resolvable type
  aliases are included. Overloads and merged declarations share one suite.
- Plain data properties, index signatures, and private/protected members do not
  get function tests.

This is a **declaration inventory**, not a type-checker-expanded runtime API graph.
Inherited members and external imports/re-exports remain in their declaring file;
they are not expanded into every consumer. Callback interfaces and callable type
aliases also get suites, because they contain signatures relevant to future
transformations. Conditional types inventory both branches. None of these entries
assert that an API exists in every runtime or infer what it throws.

Each declaration file has its own test file, including DOM, WebWorker, iterable
augmentations, and Node subpaths such as `fs/promises.test.ts`. Reference-only or
data-only files (such as `esnext.test.ts`) have a marked `it.todo` placeholder;
their callable declarations live in the referenced files' tests.

## Editing and synchronization

Replace a generated TODO with real tests:

```ts
describe("DOM", () => {
  describe("TransitionEvent", () => {
    describe("new", () => {
      it("throws TypeError", () => {
        expect(() => new TransitionEvent(undefined)).toThrow(TypeError);
      });
    });
  });
});
```

Add your `expect` import and runtime setup as needed. The initial placeholders run
in Vitest's default Node environment; real DOM/worker behavior tests will need
appropriate runtime configuration.

On regeneration:

1. Parse the declarations and existing tests before writing any output.
2. Match literal describe names at the same nesting path.
3. Preserve existing test bodies, hooks, helpers, imports, comments, modifiers,
   and custom nested suites as source text. Even an existing empty API suite is
   retained. Only newly created callable suites receive a TODO test.
4. Add missing APIs. Remove managed suites whose API disappeared, including their
   obsolete tests. Changes to overloads or signatures do not replace test bodies.
5. Keep unrelated suites/code. If a declaration file disappears, prune its managed
   suites but retain the test file and its other contents.

Commit `tests/ts/.manifest.json` and `tests/node/<major>/.manifest.json` along with
the tests. They record ownership of describe paths, enabling removal of retired
APIs without treating custom suites as generated code. On the first run against
an existing file, matching suites are adopted and unrelated suites are retained.
TypeScript manifest paths are relative to the outer library suite; reference-only
libraries keep their marked placeholder inside that suite as well.

Named Vitest imports (including aliases), namespace imports, global `describe`,
`describe.skip`/`only`, `each`, and `skipIf` are supported. Managed suite names must
be string literals with inline block callbacks. Duplicate managed names or invalid
syntax produce an error before any writes. Custom test suites inside API leaves
are opaque. Missing Vitest bindings for new code get a collision-free namespace
import; existing imports are retained.

Output ordering is deterministic; regeneration without API changes is byte-for-byte
idempotent. Sibling managed `describe` blocks are separated by a blank line,
including in existing generated files. `--check` reports stale files and exits
nonzero without writing.

## Verify

```sh
cargo test -p errorscript-lib
cargo clippy -p errorscript-lib --all-targets -- -D warnings
pnpm --filter @errorscript/lib test
cargo run -p errorscript-lib -- --check
```

The Rust integration tests exercise declaration parsing, Node naming/versioning,
first-run adoption, real assertion preservation, overloads, additions/removals,
retired files, custom code, import aliases, CRLF, idempotency, and invalid inputs.
Passing dummy Vitest tests indicate scaffold validity, not runtime error coverage.
