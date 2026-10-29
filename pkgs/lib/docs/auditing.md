# Callable audits

The audits support three outputs: complete scoped error sets for TypeScript libs
and `@types/node` `Returns<Value, Error>` annotations; precise argument-effect
dependencies for language design; and reproducible failure mechanisms. Each
callable gets a primary-source proof and minimal runtime repros. The two
OpenCode agents are discovered from `.opencode/agents/`: `lib-callable` researches
one entry; `lib-coordinator` schedules/reviews batches. They inherit the selected
model. Start with `/lib-audit 8 callables in ts/es2015.reflect`, or select
`lib-coordinator` directly. The project enables two subagent levels so a delegated
coordinator can also launch workers.

## Queue

From the repository root (use the repository's mise toolchain):

```sh
mise run install/pnpm
mise run --quiet //pkgs/lib:next 8
mise run --quiet //pkgs/lib:next 4 --kind ts --lib es2015.reflect
mise run --quiet //pkgs/lib:next 8 --kind node --node-version 24 --lib fs/promises
```

Tasks live in `pkgs/lib/mise.toml`, never `package.json` scripts. The `next` task
forwards arguments to `cargo run --quiet -p errorscript_lib -- --next`. With mise's
`--quiet`, stdout is one JSON array, diagnostics go to stderr, errors exit nonzero,
and an exhausted scope returns `[]`. No declarations or installed type packages
are needed to list existing tests.

`--out DIR` selects the test root (default `pkgs/lib/tests`). Manifests establish
which describe paths are callables; Oxc parses their actual test files to detect
literal `it("TODO", ...)` / `test("TODO", ...)`, including aliases, namespace
imports and modifiers. Comments, strings, custom suites, and reference-only
`it.todo("No callable declarations in this file")` are not queue items. Multiple
TODOs in one callable yield one task; a partially implemented callable remains
pending. Malformed inspected files/manifests fail rather than being silently
omitted; scanning stops once the requested batch is full.

Records contain:

- `id`: output-relative test filename + `::` + JSON describe path; stable across edits.
- `kind`, `lib`, and (for Node) `node_version`: declaration scope.
- `test_file`: absolute path; `describe_path`: exact nesting; `line`: first TODO, 1-based.
- `evidence_file`: path relative to `pkgs/lib/docs/evidence`.

### Run records

Checkpoint/report pairs use `YYYY-MM-DD-<scope>-batch-NNNN.{json,md}`:
`^\d{4}-\d{2}-\d{2}-(all|ts|node[0-9]+)-batch-[0-9]{4}\.(json|md)$`.
`scope` reflects the configured selection: `all` for unrestricted/mixed kinds,
`ts` for TypeScript libraries, or `node24` (etc.) for a pinned Node major.
Other filters remain in the JSON. The sequence starts at `0001` per date/scope;
allocate the next unused sequence under sole coordinator ownership. Resume the
same ID; never rename it when budget, concurrency, or selected tasks change.
The JSON `run_id` equals the filename stem. Numbers identify batches, not queue
offsets: pending entries move and cross-file selection can be noncontiguous.

Persist checkout paths as repository-relative POSIX paths, without `..`, home
directories, or worktree prefixes. In particular, convert queue `test_file` values
to `pkgs/lib/tests/...` before checkpointing and resolve them against the current
repository root on resume. Keep the documented `evidence_file` base (evidence-root
relative) and stable queue IDs unchanged. Commands should use repository-relative
paths or their explicit package working directory. Describe external tools by
version and invocation (e.g. `mise exec -- node`), and research by pinned URLs,
not machine-specific installation paths. Temporary diagnostic output is ephemeral,
not a durable checkpoint dependency.

Results are lexicographically ordered by ID. Pass the last reviewed ID verbatim
as `--after '<ID>'` for the next page, even after its TODO has been replaced. This
is an exclusive cursor, not an offset or a reservation. One coordinator owns a
checkout; one worker owns a test file at a time. Preserve a checkpoint/blocker list
under `docs/runs/` so unresolved entries skipped by the cursor are revisited. A new
full scan without `--after` finds pending work, including newly added entries that
sort before an old cursor. Never rename TODO to hide an unresolved task.

Evidence paths use `ts/<lib>/<callable>.md` or
`node/<major>/<lib>/<callable>.md`. Callable segments are lowercased, percent-encoded
for filesystem safety, and joined with `#`; the TS library wrapper is omitted.
Case-fold collisions or long filenames receive a deterministic suffix based on
the exact path. Always use the returned path; quote `#` paths in shell commands
and percent-encode `#`/`%` when making Markdown links.

Example: `ES2015.Reflect > Reflect > apply` maps to
`pkgs/lib/docs/evidence/ts/es2015.reflect/reflect#apply.md`.

## Evidence and error contracts

Use this outline for each evidence document:

```md
# <full describe path>

- ID: <queue ID>
- Test/declaration: <paths, signatures/overloads, declaration version/revision>
- Runtime mapping: <actual built-in implementation>
- Status: complete | blocked
- Scope: <runtime/version; declared inputs vs unchecked inputs; host/state assumptions>
- Contract candidate: <supported syntax; unresolved dependency forms listed separately>

## Conclusion

<Intrinsic type union and dependent effects by channel; annotation-delivery gaps.>

## Mechanisms

| Trigger / input domain | Dependency / operation | Outcome / channel | Proof / repro |
| --- | --- | --- | --- |
| <guard; declared / unchecked / state-dependent> | <argument/this path + get/set/call/trap/etc.> | <intrinsic type, propagated effect, caught/replaced effect, or effect avoided; channel> | <source anchor + scenario name or blocker> |

## Proof

<For every branch: primary URL + spec anchor or pinned source revision/path/lines,
short verbatim quote, and reasoning. Follow called abstract operations too.>

## Repros and validation

<Each test name, triggering condition, error channel, required projects,
exact commands/results and actual Node/Chromium versions.>

## Open questions

<Unproved paths or missing runtime support; omit if none.>
```

Use ECMA-262 for ECMAScript, ECMA-402 for Intl, normative Web API and Web IDL
algorithms for browser behavior, and matching Node/engine source for implementation
behavior. Local research clones: `~/code/clones/ecma262`, `~/code/clones/chromium`,
`~/code/clones/node`. Record `git rev-parse HEAD` and permanent source links; a local
path alone is not a portable citation. Verify the clone's version before relying
on it for a specific runtime. A spec's `?`/abrupt-completion propagation is part of
the proof, not just explicit “throw” steps. Absence from MDN's exceptions list is
not a proof of `never`.

Consider invalid inputs and receivers even when a TS cast is required to reach
them, but label their reachability separately from declared inputs. A cast alone
does not establish that a mechanism is unreachable for typed callers (structural
objects may have accessors/proxies). Do not automatically add unchecked-only
diagnostics to a typed contract; record the annotation-policy decision or gap.
Identify errors caused inside the callable, not by evaluating arguments,
reading an unrelated getter, or constructing an invalid fixture before the call.
Do not replace the built-in under test. Normal algorithmic failures belong in the
contract; process termination/OOM and arbitrary monkey-patching do not provide
useful per-callable error contracts. Document any host assumptions, including
differences from the existing console annotations.

`Returns` covers synchronous, rejected, and iteration-time effects. Error-first
callbacks and emitted error events are delivery channels, not automatically throws
from the initiating call. An arbitrary callback/getter/proxy can throw any JS
value: express that dependency/unknown in the candidate instead of pretending a
finite list of `Error` classes is exhaustive. For example, `Reflect.apply` has
intrinsic validation errors and propagates the target's thrown value. Show
representative propagation with identity assertions and cite the propagation
algorithm. A declaration-only user callback type has no unique runtime
implementation; retain its TODO with a precise blocker if no justified contract
and repro can be established.

Coverage is by **distinct mechanism/dependency**, not just error class. Record
exact paths (including nested/returned objects), operation, guard and handling;
distinguish property reads/writes from invocation and synchronous effects from
rejection, iteration, callback delivery, events and later callback throws. Unknown
property sets may use a quantified path such as `options.* (enumerable own keys)`.
Record unsupported dependency forms in prose, not invented compiler syntax.

Use named scenarios and `it("throws TypeError", ...)` for intrinsic types or
`it("propagates supplied effect", ...)` for arbitrary effects. One identity
sentinel per propagation path is enough unless the implementation branches on its
type. Target proxy keys and forward other accesses; a trap throwing on every read
does not identify a dependency. Keep negative controls proving an effect is
avoided, boundaries, or reachability, not broad success-behavior suites. Collapse
invalid values only when source proves the same mechanism; preserve distinct
properties, guards, operations, platforms and channels even with identical types.

Reuse pinned shared proofs under `docs/evidence/shared/` and shared fixtures after
checking each caller's reachability, depth/options and catch behavior. Aliases may
reuse an implementation audit, but still map every declaration/overload and scope.
Read only relevant proofs/source ranges; do not copy entire transitive audits.
Completeness is source-mechanism coverage within stated assumptions, not enumeration
of every possible supplied value. Derive unions from the table; `unknown` does not
replace dependency research. Keep current repro descriptions and counts in sync.

A `.toThrow(Error)` assertion also accepts `SyntaxError`; assert the exact constructor
with `toHaveExactConstructor`. Omit redundant error names, message text, Node
codes (including absence checks), and diagnostic payloads by default. Retain a
detail only when it proves the intended failure branch rather than an incidental
same-type fixture failure; explain that purpose locally. `DOMException.name` may
identify a distinct Web API error category and should be retained. Await `.rejects`
for rejection cases and consume a generator/iterator
to observe deferred effects; the evidence must identify that channel even though
the case naming convention remains `throws ...`. Non-Error propagation can use
`it("propagates supplied effect", ...)` with an identity assertion; existing
`throws non-Error value` cases remain valid.

Only a proved empty error set gets exactly `it.skip("never throws")`. This marker
records a reviewed conclusion, not empirical coverage. Otherwise remove the TODO
only after the required repros are verified. A missing host/feature or an unproved
branch leaves a TODO and blocked evidence; a green run of skipped tests is not proof.

### Shared repro utilities

Reuse `tests/support/errors.ts` for synchronous repros in either runtime:

```ts
expectThrown(() => call(invalid), TypeError);
expect(captured).toHaveExactConstructor(ReferenceError);
await expect(result).rejects.toHaveExactConstructor(TypeError);
expect(captureThrown(() => call(inputWithThrowingGetter))).toBe(sentinel);
```

`captureThrown` fails on normal return and preserves any thrown value, including
`undefined`; it restores the `Error.stackTraceLimit` descriptor because Node's
diagnostic formatter can leak changes when supplied hooks throw. Importing this
module registers `toHaveExactConstructor`, which checks constructor identity rather
than accepting subclasses as `toBeInstanceOf`/`toThrow(Error)` do. `expectThrown`
combines capture with that matcher. For a supplied `new ErrorType(...)` sentinel,
an identity assertion alone proves propagation; do not repeat its constructor
check. Use one rejection matcher instead of Promise-instance checks followed by
several `.rejects` assertions. Prefer `captureThrown` over hand-written try/catch
with assertion failures inside the catch boundary. `withPropertyRestored` is
for synchronous temporary property changes. These helpers do not observe promise
rejections or emitted errors: await/assert those channels directly.

`throwOnGet(target, key, sentinel)` forwards unrelated reads while identifying a
specific dependency. Broad poison proxies are appropriate only for negative
controls asserting no access. Node's `node-diagnostics.ts` fixtures have a narrow,
pinned shared proof; import them only when the caller reaches that formatter.

`tests/support/node.ts` provides `runNode(args, options?)`: the current Node binary,
UTF-8 stdout, a default 10-second timeout, and failure on nonzero exit or signal.
Override timeouts explicitly for large-allocation probes. `nativeAssertions` is a
source prelude for child scripts that already import `node:assert/strict` as
`assert`; its `exact` and `same` helpers check constructors in the child realm and
propagation identity respectively. Native scripts should check constructor identity
within their own realm, never compare a serialized error to the parent's constructor.
Preserve child completion/exit checks, fixture validity, relevant callback/read counts,
delivery channels, and aggregate member types. Use explicit `spawnSync` result assertions
instead for probes of expected process termination. Keep Node-only helper imports
out of shared browser files. Callable-specific fixture setup stays visible locally;
the coordinator owns reusable helper changes and validates all affected repros.

## Runtime matrix

`vitest.config.ts` uses real Node and Playwright Chromium projects:

| Project                        | Files                                              | Runtime                   |
| ------------------------------ | -------------------------------------------------- | ------------------------- |
| `node-24` (default Node major) | `tests/node/24/**`, TS except WebWorker/ScriptHost | Node 24, checked at setup |
| `chromium`                     | TS except ScriptHost                               | Headless Chromium         |

Untagged TS repros run in both projects. Use per-test `{ tags: ["browser-only"] }`
for DOM/browser behavior, or `{ tags: ["node-only"] }` for Node-specific behavior
in shared files. The opposite project's tag definition skips that repro. If a DOM
API is proven supported in both runtimes, leave its shared repro untagged. Files
are still imported and suite callbacks still run before tag-based skipping: keep
host-specific setup inside tests, use test-local dynamic Node imports, and keep
hooks scoped to appropriate runtime suites. Do not add a module-wide browser tag
to a mixed file. Managed names and paths must stay unchanged; describe options
objects are supported by synchronization.

Browser Mode runs in a Window. WebWorker repros must execute the callable inside
an actual dedicated/shared/service worker as appropriate, with a browser test
bridging results. Assert exact types/identity in the worker realm and serialize
the result, since structured cloning can discard error details. Terminate workers
and revoke Blob URLs/registrations. Put reusable fixtures in `tests/support/` and
coordinate shared helper changes. ScriptHost needs a real compatible host/project
before resolving its TODOs. Vitest mocks blocking dialogs by default; use an
appropriate native browser fixture for those APIs rather than proving the mock.

Install and run:

```sh
mise run //pkgs/lib:browser/install
# On a fresh Linux CI image: mise run //pkgs/lib:browser/install --with-deps
mise run //pkgs/lib:test/node
mise run //pkgs/lib:test/browser
mise run //pkgs/lib:test/ts
```

During an audit always filter by file AND callable; Vitest 5 joins suite names
with `>`. Escape regex metacharacters and include the trailing test boundary:

```sh
mise run //pkgs/lib:test/ts tests/ts/es2015.reflect.test.ts \
  -t '^ES2015\.Reflect > Reflect > apply > (?!.*TODO$)' --maxWorkers 1
# Add --project node-24 or --project chromium when only that runtime is required.
```

`testNamePattern` excludes literal TODOs by default so scaffold tests don't count
as successful repros. CLI `-t` overrides it; exclude TODO explicitly and include
propagation scenarios and negative controls, not just `throws` names. Check actual
executed counts. `passWithNoTests: false` catches empty file selections. Isolation
stays enabled, `.only` is rejected, retries are disabled, and workers are bounded.
Always close files/sockets/servers, restore modified state, and avoid external
network or timing-dependent failures; create deterministic local conditions.

To add a Node line, generate `tests/node/<major>`, run the matching Node binary,
and set `ERRS_LIB_NODE_VERSION=<major>`; the config selects only that inventory and
names the project `node-<major>`. This variable does not install/switch Node.
Chromium and Playwright versions come from the pinned dependency/lockfile. Record
the actual browser version in evidence. For another engine or a necessary flag,
the coordinator updates the config/dependencies and these instructions together,
then verifies the affected projects. Workers should return a concrete config
request instead of racing to edit shared files.

For a reviewed batch, run touched files together and check synchronization once:

```sh
mise run //pkgs/lib:check
# When the Rust tooling changes:
mise run //pkgs/lib:test/rust
mise run //pkgs/lib:lint/rust
```

The durable TS annotation transform currently lives in
`subs/errs/tools/gen-libs/errs.mjs`; follow that submodule's instructions when
integrating accepted contracts and compiler fixtures. Passing runtime repros does
not by itself update declarations. Record annotation-delivery gaps explicitly,
especially for Node packages and callable forms the compiler cannot yet annotate.
