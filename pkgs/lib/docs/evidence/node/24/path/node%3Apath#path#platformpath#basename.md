# node:path > path > PlatformPath > basename

- ID: `node/24/path.test.ts::["node:path","path","PlatformPath","basename"]`
- Test: `pkgs/lib/tests/node/24/path.test.ts`, only the indicated managed suite.
- Declaration: installed `@types/node@24.19.2`, `path.d.ts:137`: `basename(path: string, suffix?: string): string;`. One signature, no overloads. `PlatformPath` is at lines 67–184; `posix` and `win32` are both `PlatformPath` (178,183), the exported `path` is `path.PlatformPath` (186–187), and `node:path` re-exports it (189–192). The `path/posix`, `path/win32`, and `node:` prefixed aliases use the same declaration (1–8,193–200).
- Runtime mapping: the built-in values inhabiting this interface are **both** `path.posix.basename` and `path.win32.basename`; both execute on Linux. The interface is not itself a function. This audit covers those built-in implementations, not arbitrary structural user implementations of `PlatformPath`. The separately managed `path > basename` suite is unchanged.
- Status: complete under the assumptions below.
- Contract candidate: **`Returns<string, unknown>`** over the full runtime input domain. It includes intrinsic `TypeError | ReferenceError | RangeError | Error` plus dependent/arbitrary diagnostic effects; the finite union alone is unsound. With primitive string `path` and absent/undefined/primitive string `suffix`, the error component is `never` under these assumptions. This is a candidate, not an annotation; Node annotation delivery is deferred.

## Conclusion

All effects are **synchronous at the basename call**:

- `TypeError`, `ERR_INVALID_ARG_TYPE`: non-string path, or supplied non-undefined non-string suffix. Suffix is validated first, even for empty paths and suffixes longer than the path. There is no object-to-path coercion; boxed strings fail.
- Uncoded `TypeError`: diagnostic property/proxy operations and name conversion, e.g. revoked proxy, nonobject constructor used as RHS of `in`, Symbol name, or a conversion returning an object.
- Uncoded `ReferenceError`: a genuine module namespace has an uninitialized `constructor` export when the diagnostic reads it.
- Uncoded `RangeError`: custom inspection hooks return each other's objects normally, causing Node's recursive diagnostic traversal to overflow.
- Exact public base `Error`, `ERR_INTERNAL_ASSERTION`: namespace formatting reached through a non-string function name bypasses the usual inspection depth cutoff; a namespace export's custom hook throws a non-ReferenceError, which the formatter replaces with the internal assertion error.
- Arbitrary values from getters, proxy traps, name coercions, inspection hooks and reached collection iteration can propagate. Current repros use one identity sentinel per dependency, not a finite list of supplied error classes. There is no intrinsic path-parser SyntaxError. Namespace formatting can instead replace supplied failures with the assertion Error above.

No promise rejection, returned-iterator error, error-first callback, or event channel exists. Diagnostic `for...of` iteration, if reached, occurs synchronously inside basename. Awaiting imports in fixtures is setup, not the callable's channel. The receiver is ignored; no filesystem access, path existence validation, suffix regular expression, normalization, cwd lookup, or platform syscall occurs in either basename implementation. Invalid UTF-16/path syntax is just string data.

Scope: Node **24.19.0**, V8 **13.6.233.17-node.51**, Linux x64, ordinary unmodified built-ins and inspection defaults, writable default `Error.stackTraceLimit`, sufficient memory. Excludes process termination/OOM and arbitrary global monkey-patching. Both POSIX and Windows algorithms are tested without requiring Windows. No other Node version or custom engine build is certified.

## Proof

### Revisions and mapping

All Node links below pin **cdc1b38d40cb567b7ad0b39c86addf830a0af0ae**. The local Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; all reads used `git show` at the pinned revision, not the checkout. [`src/node_version.h:25–27`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines 24.19.0. Runtime `process.binding("natives")` contents for `path`, `internal/validators`, `internal/errors`, and `internal/util/inspect` were each byte-for-byte equal to the pinned source.

1. [`lib/path.js:1706–1713`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path.js#L1706-L1713): `posix.win32 = win32.win32 = win32;`, `posix.posix = win32.posix = posix;`, and `module.exports = isWindows ? win32 : posix;`. The two platform objects are concrete built-ins, not unrelated implementations inferred from an interface name. [`lib/path/posix.js:3`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path/posix.js#L3) exports `require('path').posix`; [`win32.js:3`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path/win32.js#L3) analogously exports `win32`. Tests assert these aliases and the host-selected export.

2. **Entire basename algorithms.** Windows [`path.js:899–980`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path.js#L899-L980) and POSIX [`1472–1545`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path.js#L1472-L1545) begin `if (suffix !== undefined) validateString(suffix, 'suffix'); validateString(path, 'path');`. Thereafter both use only primitive string lengths, numeric index arithmetic/comparisons, primordial `StringPrototypeCharCodeAt` and `StringPrototypeSlice` (imports at 24–40). No `this` read or user hook occurs on valid strings. The Windows-only drive-prefix check uses numeric ASCII comparisons in `isWindowsDeviceRoot` (86–89); its separator helper (65–67) recognizes slash and backslash. POSIX recognizes only slash. Both suffix branches handle exact equality, matching/mismatching suffix scans, trailing separators, end correction, and slice; the absent/empty/too-long suffix branch performs a backward separator scan and returns empty or a substring. These loops are bounded by input lengths; outputs do not exceed the already-existing input. No data-dependent built-in error remains after validation (excluding OOM).

3. **Both validator sites.** [`validators.js:24–34,165–168`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L24-L34) imports **`ERR_INVALID_ARG_TYPE: { HideStackFramesError: ERR_INVALID_ARG_TYPE }`**, and its wrapped `validateString` does `if (typeof value !== 'string') throw new ERR_INVALID_ARG_TYPE(name, 'string', value);`. Thus this really uses the hide-stack constructor, not the ordinary same-code constructor. The `typeof` test does not coerce objects, unwrap proxies, or invoke hooks. [`errors.js:1389–1459`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1389-L1459) registers `TypeError, HideStackFramesError` and builds `. Received ${determineSpecificType(actual)}`. Its assertions on argument names and expected types are satisfied by the internal fixed strings; they are not additional input-validation branches.

### Diagnostic effects

4. [`errors.js:995–1043`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1043) handles null/undefined and primitive types directly. For functions it interpolates `value.name`; for objects it reads `value.constructor && 'name' in value.constructor`, then interpolates `value.constructor.name`, or falls back to `inspect(value, { depth: -1 })`. Repeated getters, `has`/`get` proxy traps, and name coercion may throw any value. Revocation/invariants, nonobject RHS of `in`, Symbol-to-string and failed ToPrimitive produce intrinsic TypeError. Arbitrary objects are not simply converted to path strings; these operations are exclusively failure diagnostics.

5. **TDZ is reached inside the call.** The first `value.constructor` read above invokes namespace `[[Get]]` on the fixture's real self-import namespace, before `export let constructor` initializes. [`V8 module.cc:405–425`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L425) checks a hole-valued export and executes `NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name)`. There is no throwing user getter or fixture argument expression. Each fixture catches only around the exact platform basename invocation, for both argument positions, then successfully finishes importing.

6. **Inspection hooks and recursion.** [`inspect.js:354–409`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L354-L409) merges options and calls `formatValue(ctx, value, 0)`. [`857–932`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L857-L932) unwraps proxies natively (no general trap traversal here), reads `value[customInspectSymbol]`, and calls it via `FunctionPrototypeCall`. Non-string results use `return formatValue(ctx, ret, recurseTimes);` **before** the `seen` cycle check or depth cutoff. Two hooks returning one another's objects thus overflow without either supplied hook throwing or recursively calling itself. [`V8 isolate.cc:1897–1913`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1897-L1913) constructs the stack-overflow exception with `range_error_function()` and throws it. This is input-induced traversal failure, not a pre-exhausted caller stack. Direct hook throws propagate unchanged.

7. **Namespace depth bypass and Error.** [`inspect.js:1050–1056,1121–1160`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1050-L1160) formats a function's base before the depth guard. [`getFunctionBase:1300–1336`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1300-L1336) uses `formatValue(ctx, value.name)` for a non-string name, **omitting recurseTimes**. `undefined > -1` is false and incrementing undefined gives NaN; the normal depth cutoff is bypassed. A legal own configurable function `name` containing a namespace therefore reaches [`formatNamespaceObject:2003–2021`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2021). Its exported value's hook throws 17; the catch executes `assert(isNativeError(err) && err.name === 'ReferenceError')`. [`internal/assert.js:8–12`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12) throws `new ERR_INTERNAL_ASSERTION(message)`; [`errors.js:1377–1383`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383) specifies base `Error`. By contrast, directly returning that namespace from the hook has recurseTimes 0, so `0 > -1` stops before invoking the namespace formatter: the tests verify ordinary `ERR_INVALID_ARG_TYPE` for this control. Namespaces are obtained from Node's native loader, not a transformed Vitest namespace.

8. **Other inspection dispatch and supplied code.** [`inspect.js:657–714,935–1186`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L935-L1186) determines constructors/prototypes, keys and brands before some depth checks. Proxy/prototype/descriptor operations, custom constructor names and getters remain dependent effects, not assumed absent at depth -1. `Symbol.toStringTag` getter errors and `isInstanceof` errors are locally caught (624–630,950–954). `getKeys` (800–825) accepts namespace TDZ ReferenceError but asserts on unexpected failures. Brand dispatch handles arrays, sets/maps, typed arrays, collection iterators, functions, RegExp, Date, errors, buffers/views, promises, weak collections, namespaces, boxed primitives, URLs and ordinary/external objects. It does not turn promise state inspection into rejection delivery. Date ISO conversion is guarded with `NumberIsNaN(DatePrototypeGetTime(value))`. RegExp reconstruction at 1066–1069 receives a genuine branded RegExp; [`V8 builtins-regexp-gen.cc:1351–1376`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/builtins-regexp-gen.cc#L1351-L1376) copies internal source and flags, not hostile `source`/`flags` properties, so this is not an intrinsic malformed-pattern SyntaxError path. Error-stack/message formatting has local catches (1471–1500,1667–1733), so not every getter throw necessarily escapes. Detached ArrayBuffer conversion is caught (2068–2073); reconstruction of detached typed arrays can instead throw TypeError (1013–1021). These are diagnostics, not path parsing.

   The function-name depth bypass can also reach collection formatting: [`inspect.js:2128–2166`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2128-L2166) uses `for (const v of value)` / `for (const { 0: k, 1: v } of value)`. Own iterator methods, next/value getters, close operations and nested formatting can execute supplied code synchronously. Repros for real Set and Map with an own throwing iterator assert sentinel identity. General property formatting (2282–2325) uses descriptors and recursively formats values; ordinary getters are not called with the default getters=false, but dedicated special-property reads/hooks above still are. The outer formatter catch at 1182–1185 handles recognized stack overflows and rethrows other failures; it does not surround the early custom-return recursion. A finite error union would conceal these supplied effects even if every intrinsic class were enumerated.

9. **Stack state and exact constructor.** [`errors.js:407–425,464–483,536–546,577–581`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L407-L546) establishes the public `.constructor` getters and the hide-stack wrapper. It sets `Error.stackTraceLimit = 0; super(...args); Error.stackTraceLimit = limit;` without finally. Diagnostic failure leaves the limit zero, so the outer `hideStackFrames` skips `ErrorCaptureStackTrace` and executes `throw error;`, preserving even primitive/frozen-value identity. Every test call and native namespace fixture restores the original descriptor in finally. Nonwritable/nondefault global stack configuration is outside the stated assumptions and can change propagation through stack recapture; it is not used as a repro here.

### ECMA-262 cross-check

Local spec revision **40ce6f206b5669e69bf010a38267ae24a2843404**, [pinned spec source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html):

- `sec-string.prototype.charcodeat` (36192–36210): out-of-range positions “return *NaN*”; `sec-string.prototype.slice` (36769–36786): “Return the substring”. Here receivers are validated primitive strings and indices are internal numbers; their coercions cannot run supplied code.
- `sec-ordinaryget` (13337–13360): “Return ? Call(_getter_, _receiver_).” Proxy `[[Get]]` / `[[HasProperty]]` (16249–16335) perform “? ValidateNonRevokedProxy” and `? Call` the traps, with explicit TypeError invariant checks. `sec-relational-operators-runtime-semantics-evaluation` specifies TypeError for a nonobject RHS of `in`.
- `sec-tostring` (5653–5675): “If _arg_ is a Symbol, throw a *TypeError* exception.” `sec-toprimitive` (5027–5055): “Let _result_ be ? Call(_exoticToPrimitive_, _input_, « _hint_ »)” and rejects object results. This explains intrinsic conversion TypeErrors and arbitrary propagation separately.
- `sec-module-namespace-exotic-objects-get-p-receiver` (15706–15733) calls module environment GetBindingValue; `sec-module-environment-records-getbindingvalue-n-s` (11712–11736): “If the binding ... is an uninitialized binding, throw a *ReferenceError* exception.”
- `sec-getiteratorfrommethod` (7064–7078): “Let _iterator_ be ? Call(_method_, _obj_).” `sec-iteratornext` (7128–7150) propagates throw completions and rejects nonobject results. Those abstract operations run within diagnostic `for...of`, not on an iterator returned by basename.

## Mechanisms and current repros

Both POSIX and win32 implementations and both argument positions are tested in
node-24. All escaping effects are synchronous; iteration below is internal to the
call, not a returned iterator. Numbered proof items above support this map.

| Trigger / input domain | Dependency / operation | Outcome | Proof / repro scenario |
| --- | --- | --- | --- |
| Declared primitive strings, optional suffix | Bounded primitive string operations; no receiver read | No supplied effects or intrinsic errors within scope | §1–3 / `maps platform exports and accepts strings without receiver checks`; alias checks and poison receiver |
| Missing/nonstring path or defined nonstring suffix (unchecked) | Suffix validation precedes path validation | TypeError; invalid suffix avoids path diagnostic getter | §2–3 / `argument validation and suffix-first order` |
| Either rejected argument reaches diagnostics (unchecked) | Argument `.constructor` get / `has name` / name get and coercion; function `.name`; custom-inspection get/call | Supplied effect propagates unchanged | §4,6 + [shared proof](../../../shared/node24-invalid-argument.md) / named `invalid path/suffix diagnostic: …` cases |
| Invalid diagnostic proxy/constructor/name | Revocation, `in`, ToPrimitive/ToString | TypeError | §4 / `diagnostic operations` |
| Namespace binding uninitialized | Argument `.constructor` get | ReferenceError | §5 / `uninitialized namespace constructor export` |
| Hooks return an object cycle | Recursive inspection of returned objects | RangeError | §6 / `custom inspection return cycle` |
| Hook returns function whose name is namespace | Nested inspection catches supplied export-hook failure | Replaces value with Error; direct namespace return stops at depth guard | §7 / `namespace formatting through function name` |
| Same depth bypass reaches Map/Set | Returned function `.name[Symbol.iterator]` call | Supplied effect propagates, synchronously | §8 / `diagnostic collection iteration propagation` |

Typed primitive-string behavior and the full unchecked-input fallback are separate
annotation choices; retain this distinction rather than treating `unknown` as a
dependency model. Ordinary basename-output tables were removed, while negative
dependency, ordering, platform and alias controls remain. Latest counts/results:
`pkgs/lib/docs/runs/2026-10-29-all-batch-0001.json`.

## Original audit validation (historical)

The commands/results below describe the original 22-test audit, before mechanism
refactoring. Source hashes and implementation mapping remain applicable.

Commands from repository root:

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- node -p 'JSON.stringify(process.versions)'
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise run //pkgs/lib:test/node tests/node/24/path.test.ts -t '^node:path > path > PlatformPath > basename > ' --maxWorkers 1
git diff --check -- pkgs/lib/tests/node/24/path.test.ts 'pkgs/lib/docs/evidence/node/24/path/node%3Apath#path#platformpath#basename.md'
```

The anchored full managed path has no regex metacharacters requiring escaping and includes the trailing test boundary. Final run at **13:04:58**, Vitest **5.0.5**, **node-24: 1 file passed; 22 passed, 23 unrelated TODOs skipped (45 total)**. Expected `USE_MAIN_CONTEXT_DEFAULT_LOADER` ExperimentalWarning only. Actual runtime: Node **24.19.0**, V8 **13.6.233.17-node.51**, Linux x64. Unactivated shell Node was 22.23.1 and was not used for validation. Source parity check ran with `PATH=... mise exec -- node --input-type=module -e ...`, using `execFileSync("git", ["-C", homedir()+"/code/clones/node", "show", "cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/"+id+".js"])` and comparing to `process.binding("natives")[id]`; all four comparisons were true. Embedded-source SHA256s:

```text
path                  dd326ecdc2d6ad2025c4991f4b480d76a9f1d52b7f6d0988a5dc0a1d02de5209
internal/validators   dd61b9cb7af5c18b46f2029c2f06b0fc2638d1cd870fe65934966e840e88e1c2
internal/errors       305e3de72f2927cf20ef1f15a11bdd963375a2d9960bbf42544bba9875827d37
internal/util/inspect 578e96c3e776b17152dfc9331befe13fb84953575197b1bea7db78564efaa8c7
```

Development runs: the initial 20-case run had 17 pass/3 fail, exposing an incorrect assumption about primitive stack recapture and a missing stack-limit restoration in the native fixture. Reading validators' import alias resolved this; the corrected propagation/teardown run had 19 pass/1 fail solely on ReferenceError **message wording** (`219 is not defined` in the win32 case rather than text containing `constructor`). The stable exact constructor/name/no-code assertions and call-local capture remain; message wording is not contractual. The next run passed all 20; adding explicit diagnostic collection iteration coverage produced the final 22-pass run above. No success claim relies on the failed runs.

`git diff --check` passed for the two owned files. Fixtures use `mkdtemp(join(tmpdir(), "basename-"))`, native `.mjs` loading through `vm.runInThisContext` with `USE_MAIN_CONTEXT_DEFAULT_LOADER`, and recursive removal in finally. No mocks, polyfills, external network, shared configuration, helpers, dependencies, annotation tables or manifests changed.

## Open questions

No unresolved runtime path under the stated assumptions. Node annotation delivery remains coordinator-owned/deferred. The observed ReferenceError message variation is not used to infer a new error class or a new failure origin; exact constructor/name and the invocation-local catch establish the required channel/type independently of engine wording.
