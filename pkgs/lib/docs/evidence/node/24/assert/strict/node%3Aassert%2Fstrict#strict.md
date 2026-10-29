# node:assert/strict > strict

- ID: `node/24/assert/strict.test.ts::["node:assert/strict","strict"]`
- Test: `pkgs/lib/tests/node/24/assert/strict.test.ts`; managed path confirmed in `tests/node/24/.manifest.json:113–122`.
- Declaration: `@types/node@24.19.2`, `assert/strict.d.ts:73`: `function strict(value: unknown, message?: string | Error): asserts value;` (one overload). Lines 74–106 merge the namespace and export this function; lines 108–110 re-export it as `node:assert/strict`. `assert.d.ts:7` imports this same strict alias. Namespace members are not additional call signatures and are outside this audit.
- Runtime mapping: `require('node:assert/strict') === require('node:assert').strict`; its `.strict` property is the same function. This is **not** the `Assert` constructor, nor a deep comparison operation.
- Status: complete (behavioral proof and Node repros; declaration delivery outstanding).
- Contract candidate: preserve `asserts value`, with synchronous effect `AssertionError | TypeError | SyntaxError | URIError | E_message | E_stack`. In `Returns` terms the normal value is `void`; a conservative unparameterized candidate is `Returns<void, unknown>`. `E_message` includes identity propagation of supplied errors and arbitrary message getter/proxy/conversion effects; `E_stack` is the supported stack-formatting hook effect. Do not replace these effects with the finite intrinsic union.

## Conclusion

All observed effects occur **synchronously during this call**. There is no returned promise, deferred iterator, error-first callback, or error event. Internal Acorn iteration is completed during the call.

Intrinsic errors:

- `AssertionError`, code `ERR_ASSERTION`: falsy value (including omitted value), unless an earlier diagnostic operation fails or an Error message is rethrown.
- `TypeError`: message conversion cannot produce an appropriate primitive; revoked/invalid message proxy operations; diagnostic source-map file URL validation (e.g. `ERR_INVALID_FILE_URL_PATH`).
- `SyntaxError`: the diagnostic tokenizer receives an incomplete single source line, even though the caller's complete JS program is valid.
- `URIError`: malformed percent encoding in the original-source file URL, decoded while generating the assertion diagnostic with source maps enabled.

An Error-valued message (including another realm's native error or an object inheriting from Error) is thrown unchanged. Plain `Error` and supplied `SyntaxError` have individual identity repros; they must not be attributed to intrinsic error construction. Message prototype traps, coercion getters/methods and `Error.prepareStackTrace` can throw **any JS value** unchanged. No coercion or property access is performed on the asserted value: all Node objects, including proxies, are truthy. A truthy value ignores the message and receiver altogether. The function does not inspect `this` or `new.target`, has no callable receiver validation, and ignores extra arguments after forwarding them.

Scope: Node 24.19.0, Linux; ordinary initialized Node host, untampered built-in prototypes/internal modules and normal stdio. The documented stack-formatting customization is explicitly included, not treated as a mock. OOM, process termination and generic stack/argument-count resource exhaustion are excluded. Windows file URL branches were source-reviewed (same TypeError/URIError classes), not executed. There is no browser implementation to test.

## Proof

### Revisions and callable resolution

Research started in `/home/koss/code/clones/node`, whose HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; that is **not** used as runtime proof. All Node citations below are pinned to **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, resolved by `git rev-parse v24.19.0^{}`. Its [`src/node_version.h:25–27`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) says major 24, minor 19, patch 0. V8 source is the vendored V8 in that same revision, matching the runtime's `13.6.233.17-node.51` (not the unrelated Chromium checkout).

[`lib/assert/strict.js:3`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert/strict.js#L3) is exactly `module.exports = require('assert').strict;`. [`lib/assert.js:918–941`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L918-L941) says `innerOk(strict, args.length, ...args);` and `assert.strict.strict = assert.strict;`. The wrapper adds no validation, receiver access, or catch.

### Truthiness and assertion construction

[`lib/internal/assert/utils.js:55–78`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert/utils.js#L55-L78): `if (!value)`, then missing argument sets the fixed message, nullish message invokes `getErrMessage(fn)`, otherwise `else if (isError(message)) { throw message; }`. Remaining failures construct an `AssertionError` with `expected: true`, `operator: '=='`, then `throw err;`. These branches account for every `innerOk` path. There is no strict-equality comparison despite the function's name.

ECMA-262 checkout revision **`40ce6f206b5669e69bf010a38267ae24a2843404`**:

- [ToBoolean](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5084-L5100), anchor `sec-toboolean`: lists the falsy primitives then `Return *true*.` No conversion hook is called. Node has no browser `[[IsHTMLDDA]]` object.
- [`AssertionError` constructor, lines 249–385](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert/assertion_error.js#L249-L385): options are an internally created plain object, so `validateObject(options, 'options')` cannot fail. Operator `==` bypasses every strict/deep diff and error-copy branch. With a message it executes `super(String(message));`; without one it inspects `actual` and `expected` and joins them with `==`. It sets `this.code = 'ERR_ASSERTION'`, captures the stack, evaluates `this.stack`, then sets `this.name = 'AssertionError'`.
- The no-message fallback can inspect only a falsy primitive and the boolean `true`: [`inspectValue`, lines 74–87](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert/assertion_error.js#L74-L87) disables custom inspection; [`inspect.js:857–866,1967–2000`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L857-L866) routes primitives/null directly to formatting. Object getters, deep comparisons, circular traversal, and custom inspectors are unreachable here. [`colors.refresh:18–47`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/colors.js#L18-L47) chooses strings from ordinary stdio/environment capabilities; under the host assumption this adds no error class. Custom replacement stdio methods would be additional supplied-code effects, not intrinsic comparison errors.

### Error recognition, conversions and proxies

[`internal/util.js:99–104`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util.js#L99-L104): `return isNativeError(e) || FunctionPrototypeSymbolHasInstance(Error, e);`. The native branch uses V8's type predicate ([`src/node_types.cc:29–52`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_types.cc#L29-L52): `value->Is##type()`). It neither formats nor accesses the native error's message. The second branch walks prototypes and can invoke a proxy.

ECMA-262 anchors at the pinned revision:

- [`sec-ordinaryhasinstance`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L6582-L6605): `Set _instance_ to ? ... _instance_.[[GetPrototypeOf]]()`. The fixed Error constructor is callable with an object prototype; only the supplied instance's prototype chain is variable.
- [`sec-proxy-object-internal-methods-and-internal-slots-getprototypeof`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L15985-L16005): `Perform ? ValidateNonRevokedProxy(_obj_)`, `? GetMethod`, `? Call`; wrong trap result or non-extensible-target invariant mismatch throws TypeError. Revocation throws TypeError; traps/getters propagate arbitrary values.
- [`sec-string-constructor-string-value`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L36057-L36068): `Let _string_ be ? ToString(_value_)`. A primitive Symbol has a special successful String-constructor case; it must not be assumed to throw merely because the message is not declared as a symbol.
- [`sec-toprimitive` / `sec-ordinarytoprimitive`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5027-L5080): `? GetMethod` for `Symbol.toPrimitive`, `? Call`, and ordinary `? Get`/`? Call` for `toString` and `valueOf`; inability to obtain a primitive says `Throw a *TypeError* exception.` These operations explain intrinsic conversion TypeErrors and arbitrary getter/method propagation. Proxy property operations may also fail their standard invariants with TypeError. None are caught by the constructor or wrapper.

The remaining conversion/validation operations are explicit: [`sec-tostring`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5653-L5676) recursively converts the result of `? ToPrimitive` and says `If _arg_ is a Symbol, throw a *TypeError* exception.` Thus an object that converts to a Symbol differs from a primitive Symbol message. [`sec-getmethod`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L6341-L6357) rejects non-callable non-nullish methods with TypeError; [`sec-validatenonrevokedproxy`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16551-L16565) says `If _proxy_.[[ProxyTarget]] is *null*, throw a *TypeError* exception.`

The constructor sets `Error.stackTraceLimit = 0` before String conversion, restoring it only later. Tests restore its original descriptor in `finally` even when conversion fails; this is restoration of real Node state, not replacement of the callable. [`internal/errors.js:242–256`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L242-L256) checks writability (and the startup-snapshot branch) before internal assignments. An installed accessor on that setting would itself contribute arbitrary accessor effects.

### Diagnostic source extraction: SyntaxError and source-map errors

[`utils.js:36–52`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert/utils.js#L36-L52) temporarily captures one frame and calls `getErrorSourceExpression(err)` without catching. [`src/node_errors.cc:1081–1118`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.cc#L1081-L1118) reads V8's source line/position into an internal plain object.

[`error_source.js:26–58,70–159`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors/error_source.js#L26-L159) returns the raw line when source maps are off; otherwise finds a map and original line. Missing map/source returns undefined and reaches the primitive-inspection fallback above. Its tokenizer loop is `for (const token of tokenizer(code, { ecmaVersion: 'latest' }))`. The comment explicitly says: `The source line may be an incomplete JavaScript source`. Tokenization is **not** protected by a catch. Acorn [`skipBlockComment:5534–5537`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/acorn/acorn/dist/acorn.js#L5534-L5537) says `this.raise(this.pos - 2, "Unterminated comment")`; [`raise:3798–3806`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/acorn/acorn/dist/acorn.js#L3798-L3806) creates `new SyntaxError(message)` and throws it. Other invalid/truncated tokens use the same syntax-error mechanism. The repro compiles **valid complete JS before capturing** the subsequent assertion call: this is not an argument-evaluation or fixture parse error.

Acorn's other explicit throw sites do not add intrinsic plain Error/non-Error values: the [options check at 469–470](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/acorn/acorn/dist/acorn.js#L469-L470) requires `sourceType === "commonjs" && allowAwaitOutsideFunction`, neither supplied by this tokenizer call. The [invalid-template sentinel at 6030–6052](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/acorn/acorn/dist/acorn.js#L6030-L6052) is locally caught by `tryReadTemplateToken`; other exceptions there are rethrown.

Source-map paths were followed rather than assuming diagnostics cannot fail:

- [`source_map_cache.js:285–336`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/source_map/source_map_cache.js#L285-L336) catches map loading/parsing/normalization failures and stores normalized source URLs. [`findSourceMap:372–406`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/source_map/source_map_cache.js#L372-L406) returns undefined for missing/invalid maps and catches constructor errors. Parsing errors here are therefore not automatic throws from strict.
- [`SourceMap.findEntry:196–225`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/source_map/source_map.js#L196-L225) searches internal parsed entries or returns `{}`. Source data comes from JSON, not caller accessors; the unmodified class's payload getter clones it. `getSourceLine` selects only string source content, splits it into lines, or returns undefined.
- Crucially [`getOriginalSource:415–459`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/source_map/source_map_cache.js#L415-L459) executes `const originalSourcePathNoScheme = fileURLToPath(originalSourcePath);` **outside** the subsequent try/catch around `readFileSync`. Missing original files' filesystem errors are swallowed, but URL errors escape during this call.
- [`internal/url.js:1580–1597,1622–1630`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/url.js#L1580-L1630) rejects nonlocal POSIX file hosts and encoded slashes, and returns `decodeURIComponent(pathname)` when percent escapes exist. [`errors.js:1486–1491`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1486-L1491) defines those file-URL errors as TypeErrors. The [Windows branch:1472–1507](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/url.js#L1472-L1507) also rejects encoded backslashes/missing drive letters and performs the same decoding; it permits UNC hosts.
- ECMA [`sec-decode`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L30788-L30835): `If _firstOctet_ is not an integer, throw a *URIError* exception.` Truncated and invalid UTF-8 encodings also throw URIError. A normalized URL can retain `%zz`; the map is accepted on module load and decoding fails only when strict retrieves the original line. Both source-map repros capture only invocation after successful fixture loading.

### Stack formatting

`AssertionError` eagerly evaluates `this.stack`, so supported formatting hooks run **inside** strict, not when a test later inspects the error. [`internal/errors.js:131–156`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L131-L156) selects user `Error.prepareStackTrace` (including getters) and executes `return globalThis.Error.prepareStackTrace(error, trace);` without a catch. The vendored [`V8 ErrorUtils::FormatStackTrace:287–351`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/messages.cc#L287-L351) uses `ASSIGN_RETURN_ON_EXCEPTION` around the embedder/custom formatting call. Arbitrary values propagate. Default source-map stack-frame formatting catches its per-frame lookup/serialization failures ([`prepare_stack_trace.js:28–69`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/source_map/prepare_stack_trace.js#L28-L69)); this does not protect the earlier `getErrMessage` source-line path.

## Mechanism review and current repros

The failed-assertion branch that rethrows an Error-valued message now has one
identity repro rather than separate Error and SyntaxError instances. Intrinsic
SyntaxError from source-context parsing remains covered separately. Non-Error
message conversion, conversion-hook retrieval, prototype lookup and global stack
formatting are separate effect mechanisms and were retained. This is a case where
Error/non-Error handling differs, so the Error-message case must not be replaced
by an arbitrary non-Error sentinel. Current execution count: 14; latest results
are in the run checkpoint.

## Original audit validation (historical)

Required project: **node-24 only**. Each concrete test asserts `.constructor` identity, not a base-class matcher. Repeated names represent distinct branches in the same managed callable suite.

| Test name | Count | Branch/channel |
| --- | ---: | --- |
| `throws AssertionError` | 4 | Explicit message; no args; generated source diagnostic; missing generated-function map / primitive inspection. Synchronous. Also checks truthy hostile objects and ignored receiver/message. |
| `throws TypeError` | 3 | Null-prototype message conversion; revoked message proxy; source-map encoded slash (`ERR_INVALID_FILE_URL_PATH`). Synchronous. |
| `throws SyntaxError` | 2 | Intrinsic truncated diagnostic tokenizer line; identity of supplied SyntaxError. Synchronous. |
| `throws URIError` | 1 | Source-map original file URL `%zz`, synchronous. |
| `throws Error` | 1 | Identity and exact constructor of supplied Error, synchronous propagation only. |
| `throws non-Error value` | 4 | Conversion method, prototype trap, conversion getter, stack formatting callback; identity assertions, synchronous propagation. |

Executed from repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/assert/strict.test.ts \
  -t '^node:assert/strict > strict > throws ' --maxWorkers 1
```

Actual runtime: Node **24.19.0**, V8 **13.6.233.17-node.51**, Acorn **8.17.0**, Linux; Vitest **5.0.5**. The task invokes `vitest run --project 'node-*'`; config selects `node-24` for this inventory and setup checks the major. Chromium excludes Node inventory, so no Chromium run or browser tag is appropriate.

Initial run: **13/13 passed**, 1 file. Final expanded run: **15/15 passed**, 1 file, no skips/TODOs, 90ms (12:46:08). The file plus full-suite regex (no regex metacharacters in the managed name) includes the trailing test boundary and excludes other callables. Source-map configuration, Error property descriptors, temporary CJS modules/files and require-cache entries are restored/deleted in `finally`. No mocks/polyfills. No inventory generation or shared configuration changes. `git diff --check -- <owned test path> <owned evidence path>` also passed.

## Open questions / integration

No unresolved behavioral path within the stated scope. **Node declaration delivery gap:** there is no durable external `@types/node` transform in this workflow. These repros and the candidate do not annotate declarations. The coordinator must supply a durable versioned external-types transform preserving the assertion signature, aliases and unknown/parametric effects, plus compiler fixtures, before claiming annotation delivery. Installed types and annotation tables were not edited. No runner/dependency/helper change is requested.
