# node:perf_hooks > ELDHistogram > [Symbol.dispose]

- ID: `node/24/perf_hooks.test.ts::["node:perf_hooks","ELDHistogram","[Symbol.dispose]"]`
- Test: `pkgs/lib/tests/node/24/perf_hooks.test.ts`, exact managed path above.
- Declaration: `@types/node@24.19.2/perf_hooks.d.ts:782-808`, single signature `ELDHistogram[Symbol.dispose](): void`. The `node:perf_hooks` module re-exports `perf_hooks` at lines 1052-1054; no additional overload. This interface describes the actual object returned by `monitorEventLoopDelay`, not a user callback interface.
- Runtime mapping: `monitorEventLoopDelay()` creates an object with `ELDHistogram.prototype`; its disposal method calls `this.disable()` dynamically. The default disable implementation stops an interval or per-iteration native histogram.
- Status: complete.
- Contract candidate: `Returns<void, TypeError | ReferenceError | EReceiver>`, where `EReceiver` is the synchronous effect of receiver property accesses, writes, and the dynamically invoked `disable`. Without an effect parameter, conservatively `Returns<void, unknown>`. `Error` and `SyntaxError` below are propagated, not intrinsic. Node annotation delivery remains deferred; no declaration annotation was edited.

## Conclusion

Intrinsic synchronous **TypeError** covers nullish receivers, absent/non-callable `disable`, the delegated `ERR_INVALID_THIS` check, revoked/invariant-violating proxies, and failed strict state writes (including a frozen enabled histogram). Intrinsic synchronous **ReferenceError** occurs when `this.disable` reads an uninitialized exported binding through a native module namespace receiver. This is created by the engine's namespace lookup, not thrown by supplied code. Receiver getters, setters, proxy traps, or a supplied `disable` can throw **any value**, synchronously and unchanged. A finite built-in-error union alone would be unsound.

For an ordinary genuine mutable histogram using the original methods, disposal returns `undefined`, whether initially disabled, enabled, or already disposed. Both native sampling modes stop without a JavaScript exception. There are no arguments to convert, no iterator, and no promise or callback/event error delivery in the original implementation. The result of a user-provided `disable` is discarded: a promise returned by that function is not awaited or returned by disposal; any later rejection is an independent supplied-code effect, not a rejection of this method's result.

Scope: Node 24.19.0, intact built-in implementations and genuine native handles. Borrowed receivers and proxies are included, not just type-correct calls. Arbitrary replacement of built-ins, forged native handles, OOM, and native process termination are excluded under the auditing policy. No forged native handles or unsafe native probes were used. All tests dispose the real fixture in `finally`.

## Proof

### Revisions and runtime identity

- Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, **not** the audited revision. All Node source was read with `git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:<path>`.
- Audited [Node source revision](https://github.com/nodejs/node/tree/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae): [src/node_version.h:25-27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines `24`, `19`, `0`. The runtime embedded `internal/perf/event_loop_delay` text was compared byte-for-byte against this revision's file and matched (`true`). The runtime was explicitly pinned to `$HOME/.local/share/mise/installs/node/24.19.0/bin/node` (the ambient `node` was 22.23.1 and was not used for validation).
- Actual runtime: Node `24.19.0`, V8 `13.6.233.17-node.51`, libuv `1.52.1`, Linux. The pinned Node source's `deps/v8/include/v8-version.h` gives `13.6.233.17`; `deps/uv/include/uv/version.h` gives `1.52.1`.
- ECMA-262 clone HEAD: `40ce6f206b5669e69bf010a38267ae24a2843404`; citations below refer to that pinned `spec.html`, with algorithm anchors for navigation. These language operations explain the ordinary JavaScript wrapper, not an assumed alternate Node implementation.

### Mapping and all wrapper branches

1. [lib/perf_hooks.js:30,42](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/perf_hooks.js#L30-L42) imports and exports `monitorEventLoopDelay`. [lib/internal/perf/event_loop_delay.js:83-96](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/perf/event_loop_delay.js#L83-L96) constructs the runtime receiver via `ReflectConstruct(..., [], ELDHistogram)`, initializing `this[kEnabled] = false` and `this[kHandle] = createELDHistogram(resolution, samplePerIteration)`. Constructor/options validation occurs before disposal and is not attributed to this callable.
2. [event_loop_delay.js:71-73](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/perf/event_loop_delay.js#L71-L73): the entire disposal body is `this.disable();`. There is no disposal-specific brand check, catch, await, or return expression. It gets and calls a receiver-controlled property with that receiver as `this` and discards its result.
3. [event_loop_delay.js:62-68](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/perf/event_loop_delay.js#L62-L68): `if (this[kEnabled] === undefined) throw new ERR_INVALID_THIS('ELDHistogram');`. [internal/errors.js:1548](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1548) registers it with `TypeError`. The error message argument is a constant string, not a user conversion. The next branch, `if (!this[kEnabled]) return false;`, stops without touching the handle. Otherwise `this[kEnabled] = false; this[kHandle].stop(); return true;`. Thus reads, strict writes, and the delegated call may propagate effects; Boolean testing does not invoke coercion hooks. The original enabled flag is Boolean. Defined lexical names in the wrapper do not exclude ReferenceError from a receiver's exotic property lookup (see below). No SyntaxError-producing parsing occurs in the original callable.

### Language operations and propagated effects

Pinned ECMA-262 [source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html):

- `GetValue` ([lines 4472-4489](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L4472-L4489), `sec-getvalue`) performs `? ToObject` and `Return ? ... [[Get]]`. `ToObject` ([5679-5697](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5679-L5697), `sec-toobject`) says: “If _arg_ is either *undefined* or *null*, throw a *TypeError* exception.” Other primitives are boxed; absent `disable` then fails the call check. No receiver string/number conversion is requested.
- `OrdinaryGet` ([13337-13358](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13337-L13358), `sec-ordinaryget`) follows the prototype and ends an accessor branch with `Return ? Call(_getter_, _receiver_)`. This propagates a `disable` getter's exact thrown value.
- `EvaluateCall` ([19865-19890](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L19865-L19890), `sec-evaluatecall`) says “If IsCallable(_func_) is *false*, throw a *TypeError* exception” and `Return ? Call(...)`. `Call` ([6394-6409](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L6394-L6409), `sec-call`) forwards `? ... [[Call]]`. A borrowed receiver's supplied `disable` can therefore throw an Error, SyntaxError, or non-Error without any conversion or interception. This is not a mock of disposal: the original disposal function is invoked unchanged.
- `PutValue` ([4499-4523](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L4499-L4523), `sec-putvalue`) propagates `[[Set]]` and throws TypeError when it returns false for a strict reference. The Node module starts with `'use strict'`, and class methods are strict. `OrdinarySetWithOwnDescriptor` ([13397-13429](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13397-L13429)) returns false for a non-writable property and propagates setter/define-property effects. This covers frozen enabled receivers; the repro uses a rejecting proxy instead, allowing clean teardown of the real histogram.
- Proxy `[[Get]]`/`[[Set]]` ([16292-16361](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16292-L16361), `sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver` and `...-set-p-v-receiver`) validate non-revocation, use `? GetMethod`, invoke traps with `? Call`, propagate target operations, and throw TypeError for invariant violations. `GetMethod` ([6341-6356](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L6341-L6356)) also rejects a non-callable trap. `ValidateNonRevokedProxy` ([16551-16564](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16551-L16564)) explicitly throws TypeError for a null proxy target. These apply both to the initial `disable` read and the delegated state/handle accesses. Forwarded descriptor/prototype operations can themselves invoke proxy traps; the candidate retains unknown propagation for them too.

### Native module namespace receiver: intrinsic ReferenceError

Cross-review correction, same callable audit: the earlier claim that defined wrapper lexical bindings excluded ReferenceError was incorrect. A property reference is not necessarily an ordinary object lookup.

- ECMA-262 `sec-module-namespace-exotic-objects-get-p-receiver`, [spec.html:15706-15731 at the same pinned revision](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L15706-L15731), resolves the exported string key and says `Return ? _targetEnv_.GetBindingValue(_binding_.[[BindingName]], *true*)`; an empty target environment itself throws ReferenceError. The module environment operation `sec-module-environment-records-getbindingvalue-n-s`, [11712-11735](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L11712-L11735), explicitly says: “If the binding for _name_ in _envRecord_ is an uninitialized binding, throw a *ReferenceError* exception.” Thus `this.disable` inside the original disposal function can create ReferenceError before any call to `disable`, even though `this` and the import holding the namespace are initialized.
- Matching Node/V8 implementation: [deps/v8/src/builtins/accessors.cc:242-252](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/accessors.cc#L242-L252) implements `ModuleNamespaceEntryGetter` by calling `holder->GetExport(...)`. [deps/v8/src/objects/module.cc:405-425](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L425) returns undefined for a missing export, but checks `IsTheHole(*value, isolate)` for an existing export and executes `THROW_NEW_ERROR(isolate, NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name))`. This is an intrinsic engine error, not an arbitrary exception supplied by a getter or callback.
- The repro executes a temporary `.mjs` file directly with the current Node executable. It self-imports its real native namespace and passes that namespace to the captured original disposal method before its `export let disable` declaration executes. Only `Reflect.apply(dispose, receiver, [])` is inside the catch; neither argument evaluation nor fixture initialization reads `receiver.disable`. The native namespace read therefore occurs inside disposal. The child asserts the exact constructor, name, and absent code, then initializes the export successfully and checks it after initialization. There is no Vite transformation, synthetic namespace, replacement method, or explicit fixture `throw`. The exception is caught synchronously, not observed as a rejected module import. The child disposes its histogram in `finally`; the parent removes the temporary directory in `finally` and enforces a child timeout.

### Native branches

- [src/node_perf.cc:281-305](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_perf.cc#L281-L305): `args[1]->IsTrue()` selects `IterationHistogram::Create`; otherwise `IntervalHistogram::Create`. This matches the `samplePerIteration` option in the tested release.
- [src/histogram.cc:338-350](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L338-L350) and [454-466](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L454-L466) bind `"stop"` with `SetFastMethod`. Slow/fast paths ([445-451](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L445-L451), [580-586](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L580-L586)) both call `StopHandleHistogram<T>`. That helper ([35-39](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L35-L39)) unwraps the real native handle then calls `histogram->OnStop()`. The unwrap macro ([src/base_object.h:249-254](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/base_object.h#L249-L254)) returns if the native pointer is null. No fake handle was used to exercise this internal mechanism.
- Interval `OnStop` ([histogram.cc:430-434](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L430-L434)): `if (!enabled_ || IsHandleClosing()) return; enabled_ = false; uv_timer_stop(&timer_);`.
- Iteration `OnStop` ([557-562](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/histogram.cc#L557-L562)) has the same guard and flag write, then `uv_check_stop(&check_handle_); uv_prepare_stop(&prepare_handle_);`.
- libuv [timer.c:97-108](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/timer.c#L97-L108) removes the timer from a heap/queue and returns `0`. Check/prepare stop expansion in [unix/loop-watcher.c:41-45](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/unix/loop-watcher.c#L41-L45) unlinks active handles and returns `0`; the [Windows branch:76-103](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/win/loop-watcher.c#L76-L103) likewise updates links and returns `0`. None calls JavaScript or translates a native error into a JS exception. Windows was source-reviewed, not runtime-tested. Stopping does not run a sampling callback, close the histogram, or emit an error event. Both original native receiver types are supplied by the factory, not by the caller of disposal.

## Mechanism review and current repros

The `this.disable` getter and invocation remain separate dependencies. A single
callback sentinel replaces duplicate Error/SyntaxError repros of the same
invocation. Delegated state repros now target the real `kEnabled` symbol for both
get and set, including the rejected-write TypeError case, forwarding other
operations. The source distinguishes the undefined
brand check, disabled early return, and enabled write/handle-stop path; native
sampling-mode/idempotence controls remain. Receiver TDZ ReferenceError and
TypeError mechanisms remain intrinsic, unlike supplied callback failures. Current
execution count: 9; latest results are in the run checkpoint.

## Original audit validation (historical)

Required project: **node-24 only** (the Node inventory file is not included by Chromium; no extra runtime tag is needed).

Ten tests execute under the selected callable:

1. `invalid receiver or non-callable disable > throws TypeError`: null, undefined, all primitive kinds, plain object, non-callable `disable`; exact constructor and absent code.
2. `delegated disable receiver check > throws TypeError`: a borrowed receiver carrying the original `disable`; exact constructor and `ERR_INVALID_THIS`.
3. `revoked receiver proxy > throws TypeError`: captured original disposal invoked after revocation, so the failing read occurs inside disposal; exact constructor.
4. `enabled state write rejected > throws TypeError`: proxy rejects the enabled-state write, both native modes; exact constructor.
5. `disable getter propagation > throws non-Error value`: identity of the supplied object.
6. `supplied disable callback propagation > throws Error`: separate synchronous identity and exact-constructor assertions.
7. `supplied disable callback propagation > throws SyntaxError`: separate synchronous identity and exact-constructor assertions.
8. `delegated state get and set propagation > throws non-Error value`: original disable on a proxy of a real histogram; identity assertions for state-read and state-write traps.
9. `disables both native sampling modes idempotently`: disabled, enabled, repeat, and re-enable paths for each mode, all return undefined; subsequent original disable reports false.
10. `native module namespace with uninitialized disable export > throws ReferenceError`: native Node ESM self-import namespace before lexical export initialization; exact constructor/name/no-code assertions inside the child realm. Parent checks clean exit, empty stderr, and the child's reported Node/V8 versions against its own.

All exception assertions capture exactly one synchronous invocation. Dynamic module import and histogram construction occur before the captured invocation. No promise rejection or deferred iterator error is being asserted.

Command (from `pkgs/lib`):

```sh
export PATH="$HOME/.local/share/mise/installs/node/24.19.0/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
ERRS_LIB_NODE_VERSION=24 ./node_modules/.bin/vitest run --project node-24 \
  tests/node/24/perf_hooks.test.ts \
  -t '^node:perf_hooks > ELDHistogram > \[Symbol\.dispose\] > (?:.* > )?(?:throws |disables )' \
  --maxWorkers 1 --reporter verbose
```

Vitest `5.0.5`: initial validation **9 passed, 51 skipped** (the selected pending marker plus unrelated callables). After removing only the selected TODO, the original validation was **9 passed, 50 skipped**. Cross-review rerun with the native namespace ReferenceError repro: **10 passed, 50 skipped**. Exactly one file and all ten named repros ran in `node-24`; no other callable ran. Parent and child used Node `24.19.0`, V8 `13.6.233.17-node.51`. No runner/dependency changes were required.

## Open questions

No unresolved behavioral paths within the stated genuine-native-handle scope. Node annotation delivery is coordinator work; this is a verified contract candidate, not an installed annotation.
