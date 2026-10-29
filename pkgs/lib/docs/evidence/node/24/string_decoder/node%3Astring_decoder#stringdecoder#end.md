# node:string_decoder > StringDecoder > end

- ID: `node/24/string_decoder.test.ts::["node:string_decoder","StringDecoder","end"]`
- Test: `pkgs/lib/tests/node/24/string_decoder.test.ts`, only the managed `end` suite.
- Declaration: installed `@types/node@24.19.2`, `string_decoder.d.ts:62`: `end(buffer?: string | NodeJS.ArrayBufferView): string`. One signature, no overloads. Lines 65–67 re-export `string_decoder` from `node:string_decoder`; this is the instance method, not the constructor or `write` callable. [Versioned declaration](https://unpkg.com/@types/node@24.19.2/string_decoder.d.ts).
- Runtime mapping: `StringDecoder.prototype.end` → optional dynamic `this.write(buf)` → `StringDecoder.prototype.write` → native `decode`; then optional native `flush` and JavaScript string addition.
- Status: complete under the host/state assumptions below.
- Contract candidate: **`Returns<string, unknown>` as an error-component update to the existing declaration**. The known intrinsic subset is `TypeError | Error | RangeError | ReferenceError`; receiver dispatch, getters/proxies, coercion and diagnostic inspection have arbitrary/dependent effects. A finite union would be unsound. `Throws<F>` could express effects of supplied callable `F`, not `Throws<ErrorClass>`. The return declaration itself is not certified for exotic receivers/native concatenation overflow (see below). Node annotation delivery is deferred; this audit does not edit declarations.

## Conclusion

All built-in failures are **synchronous**:

| Type | Conditions |
| --- | --- |
| `TypeError`, `ERR_INVALID_ARG_TYPE` | Defined argument is neither a primitive string nor a genuine ArrayBuffer view. Bare ArrayBuffer, boxed strings and proxies of views are not accepted. |
| `TypeError`, `ERR_INVALID_THIS` | The dynamically invoked native `write` receives a view but the receiver has no truthy native-state property. |
| Uncoded `TypeError` | Nullish receiver, missing/noncallable `write`, missing state on the final read; revoked/invariant-violating proxies; invalid diagnostic `constructor`/name conversion; a supplied `write` result that cannot be converted for addition. |
| Exact base `Error`, `ERR_STRING_TOO_LONG` | Native decoding exceeds the V8 string limit. The UTF-8 path checks input byte length even before decoding; other encodings can exceed their output length limit. |
| Uncoded `RangeError` | `end`'s JavaScript addition of the returned string and flushed suffix exceeds the string limit; or diagnostic custom-inspection return cycles overflow the stack. |
| Uncoded `ReferenceError` | Invalid argument is a native ESM namespace with uninitialized exported `constructor`; Node's diagnostic property read triggers TDZ. |
| Exact base `Error`, `ERR_INTERNAL_ASSERTION` | Diagnostic custom inspection returns a function whose non-string name is a namespace. This bypasses inspection's depth guard; an exported inspection hook throwing a non-ReferenceError value causes Node's namespace formatter assertion. |

Supplied receiver getters/proxy traps, `write` methods, diagnostic properties/inspection hooks, and conversions can throw **any JS value**. Tests distinguish supplied `Error`/`SyntaxError` identity from intrinsic errors. There is no intrinsic `SyntaxError`, encoding validation, promise, iterator, callback delivery, or error-event path. Asynchronous fixture imports do not turn these into rejections. An overridden `write` could independently return a promise or any other value; its behavior is not a native promise protocol supplied by `end`.

`undefined` skips `write`; primitive strings pass through native `write` unchanged but do not bypass `end`'s state access/flush. Invalid UTF-8 is replaced, incomplete UTF-16's single trailing byte is dropped, a remaining high surrogate is preserved, and base64/base64url flush incomplete groups. The decoder is reusable after flushing. DataView/typed-array native byte lengths are read without JS getters; detached/out-of-bounds views have zero length, not a JS detached-buffer exception.

Scope: official Node **24.19.0**, V8 **13.6.233.17-node.51**, Linux x64, normal engine flags, unmodified global built-ins, authentic uncorrupted native decoder state, sufficient memory. Borrowed/invalid receivers and supplied overrides/proxies are included up to the native boundary. Deliberately forging/detaching/corrupting the symbol-keyed native state can violate C++ `CHECK`s or memory invariants: those are process failures, not JS error contracts. No unsafe fake native-state call was made. OOM-only `ERR_MEMORY_ALLOCATION_FAILED` and fatal allocation failures are excluded by auditing policy, not classified as successful decoding. Other architectures/custom V8 flags are not runtime-certified here; big-endian UTF-16 byte swapping follows the same string-creation error path.

## Proof

### Revision and exact implementation

All Node links use **cdc1b38d40cb567b7ad0b39c86addf830a0af0ae**. Local Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, so research used `git show` at the matching revision. [`src/node_version.h:25–27`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines `24`, `19`, `0`. A runtime comparison verified that embedded `string_decoder`, `internal/errors`, and `internal/util/inspect` are byte-for-byte identical to these files at that revision.

1. [`lib/string_decoder.js:24–52,60–101,146`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/string_decoder.js#L60-L101) exports the same `StringDecoder` constructor and assigns the instance method directly. Its entire control flow is:

   ```js
   const ret = buf === undefined ? '' : this.write(buf);
   if (this[kNativeDecoder][kBufferedBytes] > 0)
     return ret + flush(this[kNativeDecoder]);
   return ret;
   ```

   No catch or async boundary exists. Ordinary property reads and the dynamic call propagate. The receiver is not checked as a class instance: a borrowed receiver inheriting authentic state can work, while nullish receivers/missing properties/noncallable `write` fail. The final state read happens even for omitted/string arguments. Custom return conversion only happens on the flush branch; flush occurs before addition's conversion, so conversion failure can leave the decoder already flushed.

2. [`write:76–87`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/string_decoder.js#L76-L87): `if (typeof buf === 'string') return buf;`, then `if (!ArrayBufferIsView(buf)) throw new ERR_INVALID_ARG_TYPE(...)`; then `if (!this[kNativeDecoder]) ... ERR_INVALID_THIS`; finally `return decode(this[kNativeDecoder], buf)`. Input type validation precedes the native-state check. `ArrayBufferIsView` is primordial, does not coerce and does not unwrap proxies. No iterator, `valueOf`, encoding-normalization or argument string conversion is used on valid views. Constructor-time unknown encoding is not an error of this method; public `encoding` is not consulted by `end`.

### Native decoder, buffers and string creation

3. [`src/string_decoder.h:32–43`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder.h#L32-L43) defines a seven-byte state, buffered-byte index 5 and encoding index 6. [`string_decoder-inl.h:10–23`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder-inl.h#L10-L23) reads those bytes directly. [`src/string_decoder.cc:259–282`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder.cc#L259-L282) obtains native state through `Buffer::Data`, `CHECK_NOT_NULL(decoder)`, and `ArrayBufferViewContents<char>`. The bindings set a return value only when `.ToLocal(&ret)` succeeds. [`node_buffer.cc:250–258`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_buffer.cc#L250-L258) requires `CHECK(val->IsArrayBufferView())`; it does not turn forged internal state into a recoverable TypeError. These checks explain the state assumption rather than silently assuming every arbitrary receiver is safe to pass into native code.

4. [`src/util-inl.h:583–600`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util-inl.h#L583-L600): `length_ = abv->ByteLength();` and native backing data/offset or `CopyContents` are used, not supplied JS properties. [`deps/v8/src/api/api.cc:9299–9312`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/api/api.cc#L9299-L9312): `if (obj->WasDetached()) { return 0; }`, otherwise typed-array/DataView internal length. [`js-array-buffer-inl.h:234–245,418–427`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/js-array-buffer-inl.h#L418-L427) returns zero for out-of-bounds resizable DataViews; typed arrays use their corresponding out-of-bounds-aware length. Tests isolate these native probes in a child process and exercise ordinary DataView and SharedArrayBuffer-backed views too.

5. [`src/string_decoder.cc:59–228`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder.cc#L59-L228) covers every encoding: UTF-8/UCS2/base64/base64url finish buffered prefixes, retain incomplete suffixes, call `MakeString`, and optionally native `String::Concat`; ASCII/hex/latin1 call `MakeString` directly. Malformed continuation handling at 78–95 and invalid UTF-8 suffix handling at 138–185 deliberately pass bytes to the replacement decoder, not a fatal decoding mode. Missing/buffered counts are bounded by the four-byte incomplete-character storage for authentic state. [`FlushData:230–255`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder.cc#L230-L255): UCS2 odd byte counts are decremented; zero buffered bytes return the empty string; otherwise `MakeString` is called and both counts reset. Valid flush data is tiny, so an independent flush-size overflow is not reachable.

6. [`MakeString:27–53`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_decoder.cc#L27-L53): UTF-8 calls `String::NewFromUtf8` only if `length <= ...v8::String::kMaxLength`; otherwise `isolate->ThrowException(node::ERR_STRING_TOO_LONG(isolate));`. Other encodings call `StringBytes::Encode`, propagating an empty result/pending exception. [`src/node_errors.h:125,306–311`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h#L306-L311) maps `ERR_STRING_TOO_LONG` to **Error**, with message `Cannot create a string longer than 0x%x characters`. The child-process repro allocates a real Uint8Array of `MAX_STRING_LENGTH + 1` outside the error catch, then catches only `decoder.end(input)`; no output allocation/large body copy is required on this branch.

7. [`src/string_bytes.cc:76–119,153–229,532–573,676–736`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_bytes.cc#L676-L736): ASCII clears high bits; latin1 copies; base64/base64url calculate encoded length; hex uses `dlen = buflen * 2`; UCS2 uses half byte length, byte swaps on big-endian hosts and copies unaligned data. These feed one-/two-byte V8 string creation. Empty failed string creation leads to `ERR_STRING_TOO_LONG` (e.g. external creation at 115–118); allocation-only failures are excluded as above. The initial `CHECK_BUFLEN_IN_RANGE` cannot add a non-OOM error on a genuine view: [`node_buffer.h:32`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_buffer.h#L32) uses Uint8Array's maximum, [`v8-typed-array.h:26,59–60`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/include/v8-typed-array.h#L59-L60) uses ArrayBuffer's maximum byte length. No valid view can exceed its backing-buffer maximum. `keep_buflen_in_range` at 236–240 clamps to `INT_MAX`; it does not throw. The `BUFFER` encoding allocation branch is not selected by an uncorrupted StringDecoder constructor.

8. Distinguish **native concatenation** from **JS addition**. [`deps/v8/src/api/api.cc:7662–7678`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/api/api.cc#L7662-L7678) says native `String::Concat` will “return the null handle instead” on overflow. Thus that particular `DecodeData` prefix/body overflow does not establish a RangeError; the binding can leave its return undefined. This is a source-established return-type caveat, not a separately validated half-gigabyte-copy repro. In contrast, `end` uses JS `ret + flush(...)`: [`builtins-string-gen.cc:484–512`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/builtins-string-gen.cc#L484-L512) sends over-limit addition to the runtime; [`factory-base.cc:869–895`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/heap/factory-base.cc#L869-L895) throws `NewInvalidStringLengthError()`, and [`factory.cc:2804–2812`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/heap/factory.cc#L2804-L2812) returns `NewRangeError(MessageTemplate::kInvalidStringLength)`. The tested string argument is constructed before the call as a legal maximum-length rope; adding one replacement character fails inside `end`, not inside `repeat` or fixture creation.

### Diagnostics and arbitrary supplied effects

9. [`lib/internal/errors.js:1389–1459,1548`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1389-L1459) registers invalid argument/receiver errors as TypeError. The argument diagnostic appends `determineSpecificType(actual)`. [`995–1043`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1043) reads function `name`, tests `value.constructor && 'name' in value.constructor`, interpolates `value.constructor.name`, or calls `inspect(value, { depth: -1 })`. Thus getters, `in` proxy traps, coercion, revoked/inconsistent proxies, and native ESM namespace binding access are reachable **inside this callable**, not during argument evaluation. [`errors.js:464–483,551–581,591–600`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L464-L483) shows message construction and the public `constructor` getter returning the base class. `end`/`write` use the ordinary error class, not the optional `.HideStackFramesError` variant; diagnostic effects propagate directly. Repros still restore the original `Error.stackTraceLimit` descriptor in `finally`, including the self-import fixture.

10. [`inspect.js:885–932`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L885-L932) obtains `value[customInspectSymbol]`, invokes `FunctionPrototypeCall(maybeCustom, ...)`, and for a non-string result executes `return formatValue(ctx, ret, recurseTimes);` **before** circular-value registration/depth checks. Hook throws propagate; two hooks merely returning each other's objects cause Node's diagnostic recursion. [`deps/v8/src/execution/isolate.cc:1897–1913`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1897-L1913) constructs the stack-overflow error using `range_error_function()` and throws it. This RangeError is generated by traversal, not explicitly thrown by either hook.

11. Namespace TDZ: [`deps/v8/src/objects/module.cc:405–425`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L425) checks the hole-valued export and uses `NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name)`. The native self-import fixture passes its namespace before `export let constructor` executes. Only the `decoder.end(namespace)` call is caught; the import completes normally.

12. Namespace assertion: direct namespace custom-inspection return is stopped by `recurseTimes > ctx.depth` at [`inspect.js:1121–1160`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1121-L1160). The test includes that control. However [`getFunctionBase:1300–1336`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1300-L1336) uses `formatValue(ctx, value.name)` for a non-string name **without the recursion counter**. `undefined > -1` is false and incrementing yields NaN, allowing namespace formatting. [`formatNamespaceObject:2003–2021`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2021) catches a property-inspection failure and runs `assert(isNativeError(err) && err.name === 'ReferenceError')`. [`internal/assert.js:8–12`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12) throws `new ERR_INTERNAL_ASSERTION(message)`; [`errors.js:1377–1383`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383) defines base Error. The hook's `17` is replaced with that exact coded Error; this is not classified as identity propagation.

### ECMAScript cross-check

Local ECMA-262 HEAD **40ce6f206b5669e69bf010a38267ae24a2843404**; [pinned specification](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html). Relevant anchors/quotes:

- `sec-arraybuffer.isview`: “If _arg_ has a [[ViewedArrayBuffer]] internal slot, return *true*.” No getter/coercion or proxy unwrapping.
- `sec-ordinaryget`: “Return ? Call(_getter_, _receiver_).” Receiver/diagnostic getters propagate.
- `sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver`: `? ValidateNonRevokedProxy`, `? Call` and invariant TypeErrors; proxy `[[HasProperty]]` similarly covers diagnostic `in`. `sec-relational-operators-runtime-semantics-evaluation` rejects a non-object right-hand side of `in`.
- `sec-toprimitive`: “Let _result_ be ? Call(_exoticToPrimitive_, _input_, « _hint_ »).” Invalid object results cause TypeError; supplied thrown values propagate.
- `sec-tostring`: “If _arg_ is a Symbol, throw a *TypeError* exception.” Applies to diagnostic name interpolation and addition of a supplied Symbol result.
- `sec-applystringornumericbinaryoperator`: `+` uses `? ToPrimitive` on both operands and `? ToString` if either is a string. This covers the supplied-write-result conversion repros, not merely string length.
- `sec-module-namespace-exotic-objects-get-p-receiver` reaches module `GetBindingValue`; `sec-module-environment-records-getbindingvalue-n-s`: “If the binding ... is an uninitialized binding, throw a *ReferenceError* exception.”

## Mechanisms and current repros

Required runtime: node-24. All errors below escape synchronously from `end`.
The Proof sections above pin the dispatch, native decoder, diagnostic and coercion
sources. The runtime-wide `unknown` fallback is not a precise dependency annotation.

| Trigger / input domain | Dependency / operation | Outcome | Proof / repro scenario |
| --- | --- | --- | --- |
| Defined nonstring/non-view buf; unchecked or typed view proxy | `this.write(buf)` native view validation | TypeError or diagnostic effects | Exact implementation / `argument validation` |
| Missing receiver/state or noncallable write (unchecked receiver) | `this.write` call; `this[kNativeDecoder]` and buffered-byte reads | TypeError | Exact implementation / `invalid receiver and write property` |
| Buf supplied; structural/overridden receiver | `this.write` get/call | Supplied effect; omitted buf avoids both operations | Exact implementation / `this.write get/call when buf is supplied; native state read always` |
| Any buf, including omitted buf | `this[kNativeDecoder]` get | Supplied effect; not guarded by buf presence | Same source/scenario; explicit symbol-key targeting |
| Native output length exceeds V8 limit; genuine buffer/state | Decode allocation/encoding | Error | Native decoder / `native UTF-8 string length limit` |
| Flush plus returned string exceeds maximum length | Addition after flush | RangeError | Native decoder / `flush concatenation length limit` |
| Invalid buf reaches formatter | Buf constructor/name/coercion/inspection operations | Supplied effect propagates | Diagnostics + [shared proof](../../../shared/node24-invalid-argument.md) / named `invalid buf diagnostic: …` cases |
| Declared Uint8Array proxy lacks native brand | Buf `.constructor` get | Supplied effect despite declared input shape | Exact implementation + diagnostics / `declared buffer-view shape without native view brand` (no cast) |
| Invalid diagnostic proxy/constructor/name | Proxy/`in`/conversion operations | TypeError | Diagnostics / `diagnostic formatting` |
| Native namespace TDZ or inspection return cycle | Buf constructor get or returned-object traversal | ReferenceError or RangeError | Diagnostics / `uninitialized namespace constructor export`, `custom inspection return cycle` |
| Function-name namespace depth bypass | Catch a supplied exported hook failure | Replaced with Error, not propagated | Diagnostics / `namespace formatting through a function name` |
| Buffered bytes and overridden write result | Result ToPrimitive/ToString on addition | TypeError for Symbol or propagated conversion effect | ECMA cross-check / `supplied write result conversion on flush` |
| No buffered bytes | Return write result directly | Result coercion avoided | Exact implementation / `avoids write-result coercion without buffered bytes` |
| Detached/out-of-bounds genuine views | Native view handling | No error on tested boundaries | Native decoder / isolated native-process control |

Receiver overrides/internal-symbol mutations need their own reachability assumptions;
they are not automatically effects of `buf`. General encoding-output tests were
replaced by the no-flush coercion control. Current counts/results are recorded in
`pkgs/lib/docs/runs/2026-10-29-all-batch-0001.json`.

## Original audit validation (historical)

The following commands/results describe the original fifteen-test audit before
shared fixtures and mechanism-based refactoring, not current test counts.

Commands from repository root:

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- node -p 'JSON.stringify(process.versions)'
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise run //pkgs/lib:test/node tests/node/24/string_decoder.test.ts -t '^node:string_decoder > StringDecoder > end > ' --maxWorkers 1
git diff --check -- pkgs/lib/tests/node/24/string_decoder.test.ts 'pkgs/lib/docs/evidence/node/24/string_decoder/node%3Astring_decoder#stringdecoder#end.md'
```

The filter is anchored to the entire managed path plus trailing boundary; this path contains no regex metacharacters requiring escapes. Runs at **13:09:14** and **13:13:31**, Vitest **5.0.5**, both: **1 file passed; 15 tests passed, 2 unrelated TODOs skipped (17 total)**. `git diff --check` passed. Native child processes use `process.execPath`, so match the Node 24.19.0 project. Runtime was Linux x64, V8 13.6.233.17-node.51. The unactivated shell's Node 22.23.1 was observed but not used for validation. The native-loader ExperimentalWarning is expected, not an error or skip.

Embedded-source verification command (passed for all three modules):

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- node --input-type=module -e 'import {execFileSync} from "node:child_process"; import assert from "node:assert/strict"; for (const path of ["lib/string_decoder.js","lib/internal/errors.js","lib/internal/util/inspect.js"]) { const key=path.slice(4,-3); assert.equal(process.binding("natives")[key],execFileSync("git",["-C",process.env.HOME+"/code/clones/node","show","cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:"+path],{encoding:"utf8"})); console.log(key+" embedded source matches"); } console.log(process.version,process.versions.v8,process.platform,process.arch);'
```

Temporary ESM files use `mkdtemp(join(tmpdir(), "string-decoder-end-"))` and are removed in finally. Data URLs need no disk cleanup. No mocks, polyfills, shared config/dependency/helper edits, inventory generation, or declaration table changes.

## Open questions

No error-path blocker under the stated assumptions. Coordinator-level Node annotation delivery remains deferred. If return-shape work is included later, account for overridden `write` and native `String::Concat`'s empty-handle overflow rather than interpreting the existing `string` declaration as a proof of a string on every runtime path. No shared configuration request.
