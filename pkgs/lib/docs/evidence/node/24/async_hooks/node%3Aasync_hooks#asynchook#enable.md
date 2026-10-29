# node:async_hooks > AsyncHook > enable

- ID: `node/24/async_hooks.test.ts::["node:async_hooks","AsyncHook","enable"]`
- Test: `pkgs/lib/tests/node/24/async_hooks.test.ts`, exact managed path above.
- Declaration: `@types/node@24.19.2/async_hooks.d.ts`, lines 152–156: `interface AsyncHook { enable(): this; }`. One signature, no overloads. Lines 617–619 re-export `async_hooks` from `node:async_hooks`, not a separate implementation. Declaration SHA-256: `31a2721bfdb4eaf3af1168396bdc307d14892074aa64801ac975aa9809a1727f`.
- Runtime mapping: non-exported class method `AsyncHook.prototype.enable`, obtained from the prototype of `createHook({})`. `AsyncHook` is an interface in declarations, but this method has a concrete built-in implementation; it is not a callback declaration.
- Status: complete (behavior/evidence only; declaration delivery remains blocked).
- Contract candidate: `Returns<this, TypeError | ReceiverReadEffects>`, where `ReceiverReadEffects` includes arbitrary thrown values from six receiver symbol-property reads and their transitive getter/proxy operations. Conservative fallback: `Returns<this, unknown>`. A finite `TypeError | Error | SyntaxError` annotation is **not** sound. With an ordinary unmodified genuine hook and valid internal state, the error component specializes to `never`.

## Conclusion

All established throwable effects are **synchronous**. Intrinsic **TypeError** arises for nullish receivers and proxy access failures (revoked proxies, non-callable `get`, invariant violations). Getter/proxy code can propagate **any value**, including Error, SyntaxError, objects, and undefined; the method does not construct Error or SyntaxError itself.

There is no receiver brand check. Non-nullish primitives and arbitrary objects can be registered. An already-registered receiver is returned without property access. Otherwise the five callback symbols and then `kNoPromiseHook` are read. Callback values are boolean-tested, not validated or invoked. Arguments are unused. Unlike `disable`, this method reads a **sixth** symbol, and a failure reading that flag occurs **after insertion** into the active hooks array. Earlier read failures can also leave partially updated counters; there is no rollback.

The method returns its receiver, not a promise or iterator. No rejection, iteration-time error, or error-event channel originates here. Subsequent async event delivery invokes registered callbacks; throwing there can terminate Node, rather than throwing from an earlier `enable` call. This is not an asynchronous `Throws` effect of `enable`. Getter/trap code executed during `enable` can itself perform arbitrary operations; its transitive thrown effects remain covered by `ReceiverReadEffects`.

Scope: Node v24.19.0 on Linux, intact built-ins/primordials/private host bookkeeping; adversarial receivers and receiver property access included. Excludes replacement of the method, global prototype/primordial tampering, corrupted native state, resource/stack exhaustion, process termination, and unsupported custom builds. Tests use genuine hooks, not mocks/polyfills. Exceptions can leave state changed, so repros use zero-callback receivers and restore access before cleanup in isolated processes.

## Proof

### Exact revisions and runtime match

Local Node clone HEAD: `510d84c330e8e7d2bb8699608163024baf54eaa6` (not the runtime revision). All Node/V8 source references below use **v24.19.0**, commit **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, matching `src/node_version.h` lines 25–27. Runtime is **Node v24.19.0 / V8 13.6.233.17-node.51**; vendored `deps/v8/include/v8-version.h` lines 11–14 gives 13.6.233.17. This reuses and independently checks the accepted `disable` audit's revision mapping.

Compared `process.binding('natives')[name]` byte-for-byte against `git show v24.19.0:lib/<name>.js` under `mise exec -- node`; all comparisons were true:

| Module | Runtime/source SHA-256 |
| --- | --- |
| `async_hooks` | `fa88748e8443544189029826955f0cdb7386473a0b7191956d7529a22457dff7` |
| `internal/async_hooks` | `08a5d05e584c48a6f51edd61e1e6cb117906d763e521e4b8dc2c7227462bbdca` |
| `internal/promise_hooks` | `1b2d7268017e67bbde07b27d51e80ff90385505b003876ee0ebc49411e7f1ae8` |
| `internal/validators` | `dd61b9cb7af5c18b46f2029c2f06b0fc2638d1cd870fe65934966e840e88e1c2` |

ECMA-262 local revision: **`40ce6f206b5669e69bf010a38267ae24a2843404`** ([pinned source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html)). Algorithm links below identify anchors, and line numbers refer to this revision. The matching Node-vendored V8 is the implementation source, not an unrelated Chromium revision. Chromium is not a required project for this Node-only entry.

### Dispatch, receiver access, and internal bookkeeping

1. [Node `lib/async_hooks.js` lines 76–107, 109–172](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/async_hooks.js#L76-L172): `createHook` does `return new AsyncHook(fns);`. Constructor callback/trackPromises validation is **not** repeated by `enable`; `ERR_ASYNC_CALLBACK` / invalid-argument errors from constructing a bad fixture are not attributable to this callable. Constructor properties are ordinary symbol-keyed data fields. The sixth symbol is defined as `const kNoPromiseHook = Symbol('kNoPromiseHook');` ([internal source line 587](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L587)).
2. `enable`, lines 114–118: `getHookArrays()` then `if (ArrayPrototypeIncludes(hooks_array, this)) return this;`. [Internal async hooks lines 274–312](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L274-L312) returns private arrays/counters directly outside delivery, or creates/reuses temporary copies during delivery: `if (active_hooks.tmp_array === null) storeActiveHooks();`. Copying private dense arrays/numeric fields introduces no user operands. [Array.prototype.includes](https://tc39.es/ecma262/#sec-array.prototype.includes), spec lines 41099–41123, compares “using the SameValueZero algorithm”; it does not coerce/access the search value. Thus membership short-circuits even an adversarial receiver. Array slice/species operations operate on ordinary internal arrays under the intact-intrinsics assumption.
3. `enable`, lines 125–131, performs five ordinary property reads, e.g. `hook_fields[kTotals] = hook_fields[kInit] += +!!this[init_symbol];`, then `ArrayPrototypePush(hooks_array, this);`. There is no catch. [GetValue](https://tc39.es/ecma262/#sec-getvalue), spec lines 4472–4496: `Let _baseObj_ be ? ToObject(_refRecord_.[[Base]]).` [ToObject](https://tc39.es/ecma262/#sec-toobject), lines 5679–5703: “If _arg_ is either *undefined* or *null*, throw a *TypeError* exception.” Other primitives box; objects remain objects. Symbol keys need no user conversion. The variables and symbols are bound, so no unresolved-reference ReferenceError path is present.
4. [ToBoolean](https://tc39.es/ecma262/#sec-toboolean), lines 5084–5101, returns false for its listed falsy values, otherwise `Return *true*.` It never calls user coercion hooks. `+!!` consequently converts a Boolean, not the original callback value, to 0/1. There is no BigInt/Symbol numeric-conversion error here. Counter updates use internal numbers. Primordial array push appends to a mutable private ordinary array; it does not inspect the appended receiver. Resource-exhaustion limits are outside the stated scope.
5. `enable`, lines 133–141: `if (prev_kTotals === 0 && hook_fields[kTotals] > 0) { enableHooks(); }`, then `if (!this[kNoPromiseHook]) { updatePromiseHookMode(); }`, then `return this;`. Thus the sixth receiver read is independently throwable even for zero callbacks; a truthy flag bypasses promise-mode updating. The array insertion is already complete when the flag is read. The test helper's finally disables the receiver after restoring reads, including this post-insertion failure branch.

### Intrinsic proxy errors and arbitrary propagation

- [Proxy `[[Get]]`](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver), spec lines 16292–16319: `Perform ? ValidateNonRevokedProxy`, `Let _trap_ be ? GetMethod`, `Let _trapResult_ be ? Call`, and target `[[GetOwnProperty]]` propagate abrupt completion. Mismatching frozen data values and non-undefined results for getter-less non-configurable accessors explicitly “throw a *TypeError* exception”. [ValidateNonRevokedProxy](https://tc39.es/ecma262/#sec-validatenonrevokedproxy), lines 16551–16567, throws TypeError for a null target. [GetMethod](https://tc39.es/ecma262/#sec-getmethod), lines 6341–6358, throws TypeError if `IsCallable(_func_) is *false*`.
- Matching [V8 `proxy-get-property.tq` lines 14–61](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/proxy-get-property.tq#L14-L61): `ThrowTypeError(MessageTemplate::kProxyRevoked, 'get');`, `GetInterestingMethod(...)`, `Call(context, trap, handler, target, name, receiverValue)`, and `CheckGetSetTrapResult(...)` implement those checks. The invariant repro's trap returns normally; V8 creates the TypeError.
- [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget), spec lines 13337–13361, propagates prototype lookup and ends an accessor read with `Return ? Call(_getter_, _receiver_).` Direct/inherited getters, handler trap getters, trap calls, and nested proxy target descriptor operations can throw any value. No catch in `enable` narrows that effect. Independent Error and SyntaxError cases assert supplied identity and exact constructor; object/undefined cases assert identity and an explicit caught flag. Both direct accessors and proxies are tested on **all six** symbol reads.

### Enabling, promise-mode updating, and delivery channels

- [Internal async hooks lines 375–399](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L375-L399): `enableHooks` increments the internal check count and calls `setCallbackTrampoline(callbackTrampoline)`. [Native setter, `src/async_wrap.cc` lines 359–366](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L359-L366) stores that known internal function; it does not invoke it or application callbacks.
- `updatePromiseHookMode` sets `wantPromiseHook = true`, chooses an internal init wrapper according to init/destroy counters, stops any prior internal hook, and installs `{ init: initHook, before: promiseBeforeHook, after: promiseAfterHook, settled: promiseResolveHooksExist() ? promiseResolveHook : undefined }`. [Counter predicates, lines 478–500](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L478-L500) read only numeric internal fields. No callbacks are invoked to make this selection. Lazy `require('internal/promise_hooks')` loads Node's bundled module, not a user-selected path.
- [Internal promise hooks lines 70–117](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/promise_hooks.js#L70-L117): `makeUseHook` validates a function, pushes it to an internal array, calls `update`, and returns a bound stop function. [validatePlainFunction, lines 484–487](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L484-L487) rejects `typeof value !== 'function' || isAsyncFunction(value)`. Every supplied value here is a known synchronous internal function, or undefined skipped by `createHook`, so its ERR_INVALID_ARG_TYPE branch is unreachable from receiver callback contents. Prior stop functions only splice internal arrays and update installed functions. `update` selects function references (`list.length > 1 ? runAll : list[0]`) and calls `setPromiseHooks`; it does not run user hooks, even if other promise hooks are registered.
- [Native SetPromiseHooks, `src/async_wrap.cc` lines 201–208](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L201-L208) forwards function-or-empty handles to [Environment::ResetPromiseHooks, `src/env.cc` lines 83–118](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/env.cc#L83-L118), which stores handles and updates contexts. [V8 Context::SetPromiseHooks, lines 7323–7365](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/api/api.cc#L7323-L7365) updates native-context slots. The build without `V8_ENABLE_JAVASCRIPT_PROMISE_HOOKS` instead fails an API check (fatal, not a JS throw); that is not the tested build. No OS-specific system-error branch exists on these paths.
- Separate event delivery is [internal async hooks lines 192–258](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L192-L258): callback exceptions are caught with `fatalError(e)`. [Lines 160–175](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L160-L175) abort or exit. Those callbacks are not called by registration. The during-delivery success repro catches its assertions inside the callback to avoid fatal delivery errors and exercises both creation and reuse of temporary hook arrays. Fixture `disable` calls finish before the child emits its marker; internal deferred promise-hook cleanup follows the accepted sibling evidence, and the parent checks normal child exit as well as the marker.

## Mechanism review and current repros

`checkReads` still targets each internal hook symbol plus `kNoPromiseHook`, in
both getter and proxy modes. Nullish/invalid receivers and proxy violations create
TypeError; supplied read effects propagate by identity. A single sentinel replaces
three duplicated value-type matrices. Already-enabled receivers avoid the reads;
the reentrant registration and callback-termination cases remain distinct channels.
The source proof above is unchanged; no callback invocation effect is inferred
merely from reading a callback slot. Current execution count: 4; latest results
are in the run checkpoint.

## Original audit validation (historical)

All six tests have `{ tags: ["node-only"] }`. Each runs real `enable` in a child using `process.execPath`, asserts in that realm, and prints a marker including its Node version only after cleanup. Parent assertions check successful child exit and the matching version marker. Ten-second child timeout bounds failures. Property failures are caught **synchronously inside the child**; no `.rejects` applies. Finally blocks disarm receiver failures and disable any registered fixture. Zero-callback failure fixtures keep counters at zero, including after late read failures. Revoked/non-callable-get fixtures fail before registration. No hook is left armed in Vitest.

| Test | Verified branches |
| --- | --- |
| `throws TypeError` | Undefined/null receivers, revoked proxy, non-callable get trap, frozen data invariant failure on all six symbols. Exact TypeError constructor/name and undefined code. |
| `throws Error` | Supplied Error identity and exact constructor, each of six direct getter and six proxy reads. |
| `throws SyntaxError` | Separate supplied SyntaxError identity and exact constructor, same twelve read branches. |
| `throws non-Error value` | Frozen object and undefined, each through all six getters and all six proxy reads; explicit caught flag. |
| `returns the receiver without brand checks` | Number/string/Boolean/BigInt/Symbol/object receivers, repeated enable, and already-registered proxy with armed throwing access. |
| `returns through hook and promise-mode transitions` | Empty/init/destroy/init+destroy/promiseResolve/before+after/trackPromises:false configurations, first and additional registration, idempotence, zero synchronous callbacks, and enabling during actual init delivery. |

Executed from repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/async_hooks.test.ts \
  -t '^node:async_hooks > AsyncHook > enable > (throws |returns )' \
  --maxWorkers 1 --reporter verbose
```

Result: **1 file passed; 6 tests passed, 29 skipped (35 total)**, Vitest **5.0.5**, project **node-24**, Node **v24.19.0**, V8 **13.6.233.17-node.51**. Verbose output explicitly listed all six successful enable repros; skipped tests are five accepted disable tests plus 24 unrelated TODOs. The exact suite name has no regex metacharacters requiring escapes; the trailing ` > ` boundary isolates this callable. Ambient shell `node` was v22.23.1; `mise exec`/the task select the verified v24.19.0 runtime. Chromium is excluded by the Node-only project mapping and was not run. No generation or declaration/shared-runner changes were made; the accepted disable suite and its evidence were preserved.

## Open questions

No unresolved behavioral paths in the stated scope. External Node declaration delivery remains explicitly blocked: a durable coordinator-owned transform for `@types/node` is not available. This document supplies a candidate contract only, not an installed annotation. No runner/configuration change is required.
