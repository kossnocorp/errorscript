# node:async_hooks > AsyncHook > disable

- ID: `node/24/async_hooks.test.ts::["node:async_hooks","AsyncHook","disable"]`
- Test: `pkgs/lib/tests/node/24/async_hooks.test.ts`, exact managed path above.
- Declaration: `@types/node@24.19.2/async_hooks.d.ts`, lines 152–161: `interface AsyncHook { ... disable(): this; }`. One signature, no overloads. Lines 617–619 re-export `async_hooks` from `node:async_hooks`; these are the same callable, not a second implementation. Declaration SHA-256: `31a2721bfdb4eaf3af1168396bdc307d14892074aa64801ac975aa9809a1727f`.
- Runtime mapping: non-exported `AsyncHook.prototype.disable`; `createHook(options)` constructs this class. The interface is not itself a runtime export or a callback type.
- Status: complete (behavior/evidence only; no declaration annotation delivered).
- Contract candidate: `Returns<this, TypeError | ReceiverReadEffects>`, where `ReceiverReadEffects` is the arbitrary thrown-value effect of the enabled receiver's five symbol-property reads. Without receiver-effect tracking use `Returns<this, unknown>`, **not** a finite `TypeError | Error | SyntaxError` list. For an ordinary, unmodified `createHook` result in valid hook state, the error component specializes to `never`.

## Conclusion

All established errors are **synchronous**. An enabled receiver can incur an intrinsic `TypeError` from proxy property access (revocation, invalid `get` method, or invariant violations), and can propagate **any** value from getters/proxy traps. `Error` and `SyntaxError` in the repros are supplied values, not errors constructed by `disable`.

There is no receiver brand check: an unregistered receiver, including `null`, `undefined`, primitives, or even a revoked proxy, is returned unchanged. Registered receivers are found by identity before any receiver properties are read. No arguments are read or converted. Callback values are boolean-tested, not invoked. There is no returned promise, rejection, iterator, or error-event delivery. Disabling the last counted hook schedules internal promise-hook cleanup; that cleanup does not invoke application callbacks or introduce an ordinary deferred error.

Scope: Node v24.19.0 on Linux, built-in implementation and intact host intrinsics/private bookkeeping. Includes adversarial receivers and property access, not replacement of `disable`, global primordial/prototype tampering, native addons corrupting internal state, resource exhaustion, stack exhaustion, or process termination. Proxy fixtures wrap genuine hooks; no built-in behavior is mocked or polyfilled. Hook callbacks throwing during independent async event delivery can terminate Node; that is not a throw/rejection from `disable` and is outside this contract.

## Proof

### Revisions and runtime match

Local Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, **not used as the runtime revision**. All Node/V8 citations below use tag `v24.19.0`, commit **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**. `src/node_version.h` lines 25–27 reports 24/19/0. Actual runtime: **Node v24.19.0**, **V8 13.6.233.17-node.51**; vendored `deps/v8/include/v8-version.h` lines 11–14 agrees on 13.6.233.17. No Chromium execution is required for this Node-only inventory entry.

Additionally compared `process.binding('natives')[name]` byte-for-byte with `git show v24.19.0:lib/<name>.js`; all three comparisons returned `true`:

| Module | Runtime/source SHA-256 |
| --- | --- |
| `async_hooks` | `fa88748e8443544189029826955f0cdb7386473a0b7191956d7529a22457dff7` |
| `internal/async_hooks` | `08a5d05e584c48a6f51edd61e1e6cb117906d763e521e4b8dc2c7227462bbdca` |
| `internal/promise_hooks` | `1b2d7268017e67bbde07b27d51e80ff90385505b003876ee0ebc49411e7f1ae8` |

ECMA-262 local revision: **`40ce6f206b5669e69bf010a38267ae24a2843404`**. Spec links below identify algorithms; pinned [spec source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html) supplies the cited text. Engine-specific proof uses Node's matching vendored V8 rather than a different Chromium HEAD.

### Dispatch, membership, and all synchronous branches

1. [Node `lib/async_hooks.js` lines 76–107, 144–172](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/async_hooks.js#L76-L172): `createHook` does `return new AsyncHook(fns);`. Constructor fields are symbol-keyed data properties; the private class contains the actual method. Exports at lines 295–308 export `createHook`, not `AsyncHook`.
2. Same file, lines 144–149: `const { 0: hooks_array, 1: hook_fields } = getHookArrays();`, then `const index = ArrayPrototypeIndexOf(hooks_array, this);` and `if (index === -1) return this;`. The search is over a private dense array. [ECMA-262 `Array.prototype.indexOf`](https://tc39.es/ecma262/#sec-array.prototype.indexof), spec lines 41125–41150: “using the IsStrictlyEqual algorithm”; it does not access or coerce the search value. Thus even a revoked proxy is harmless **if not registered**, and calling the method with `null` is not intrinsically erroneous. Primordial `indexOf` acts on the internal array, not on `this`.
3. [Node `lib/internal/async_hooks.js` lines 274–312](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L274-L312): outside delivery, `getHookArrays` returns the real private array and `Uint32Array` counters. During delivery it creates temporary copies once (`storeActiveHooks`, `ArrayPrototypeSlice`, `copyHooks`) and returns those instead. These are dense internal arrays and numeric fields; no user receiver operations occur here. `restoreActiveHooks` later copies them back. There is no normal validation failure on either branch.
4. `disable`, lines 151–159, reads the enabled receiver's `init`, `before`, `after`, `destroy`, and `promiseResolve` symbols in order, e.g. `hook_fields[kTotals] = hook_fields[kInit] -= +!!this[init_symbol];`. These symbols are ordinary symbols ([internal source lines 102–106](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L102-L106)), not private fields. Consequently receivers can expose accessors/proxy behavior here. There is no catch around those reads. `!!` applies [ToBoolean](https://tc39.es/ecma262/#sec-toboolean) (spec lines 5084–5101), which returns a Boolean without calling `valueOf`/`toString`; unary plus then converts that Boolean to 0/1. No Symbol/BigInt numeric-conversion error arises from the callback values. Subsequent arithmetic is on internal numbers. `ArrayPrototypeSplice(hooks_array, index, 1)` removes a known entry from a mutable private ordinary array with a known in-range numeric index. [Array.prototype.splice](https://tc39.es/ecma262/#sec-array.prototype.splice) operates on those private entries, not their contents; its length/species/property operations have no user-defined operands under the stated intact-intrinsics assumption.
5. `disable`, lines 161–165: only a transition from positive total to zero calls `disableHooks`; otherwise the method immediately returns `this`. Neither branch calls the hook callback values. [Enable, lines 109–141](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/async_hooks.js#L109-L141), proves the proxy fixture can actually be registered: it reads the symbols and pushes **`this`**, without a brand check. The tests arm failure only after this setup succeeds, so the observed failure is in `disable`, not fixture creation or argument evaluation.

### Property-access intrinsic and propagated errors

- [ECMA-262 Proxy `[[Get]]`](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver), spec lines 16292–16319: `Perform ? ValidateNonRevokedProxy`, `Let _trap_ be ? GetMethod`, `Let _trapResult_ be ? Call`, and target `[[GetOwnProperty]]` all propagate abrupt completion. It explicitly says “throw a *TypeError* exception” for a non-writable, non-configurable data property's mismatching trap result (and for a getter-less non-configurable accessor with a non-undefined result).
- [ValidateNonRevokedProxy](https://tc39.es/ecma262/#sec-validatenonrevokedproxy), lines 16551–16567: “If _proxy_.[[ProxyTarget]] is *null*, throw a *TypeError* exception.” This is relevant only after identity membership succeeds. [GetMethod](https://tc39.es/ecma262/#sec-getmethod), lines 6341–6358: “If IsCallable(_func_) is *false*, throw a *TypeError* exception.” A non-callable handler `get` is another intrinsic TypeError branch.
- Matching [V8 `proxy-get-property.tq` lines 14–61](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/proxy-get-property.tq#L14-L61) implements those steps: `ThrowTypeError(MessageTemplate::kProxyRevoked, 'get');`, `GetInterestingMethod(...)`, `Call(context, trap, handler, target, name, receiverValue)`, and `CheckGetSetTrapResult(...)`. The invariant repro returns normally from its trap, then V8 itself creates the TypeError. It is not a user-supplied TypeError.
- [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget), spec lines 13337–13361: prototype lookup propagates and an accessor ends with `Return ? Call(_getter_, _receiver_).` Therefore a direct/inherited getter, proxy trap lookup, proxy trap call, nested proxy target, or target descriptor operation can supply an arbitrary thrown value. No finite built-in error enumeration captures these paths. Identity assertions prove representative Error, SyntaxError, object, and `undefined` propagation for **all five** receiver-property reads.

### Last-hook cleanup, deferred work, and native branches

- [Internal async hooks lines 402–418](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L402-L418): `disableHooks` decrements `kCheck`, clears `wantPromiseHook`, calls `setCallbackTrampoline()`, then `enqueueMicrotask(disablePromiseHookIfNecessary)`. Its comment is “Delay the call ... because we might currently be between the `before` and `after` calls of a Promise.” The queued internal function only calls `stopPromiseHook()` when `!wantPromiseHook && stopPromiseHook`; a subsequent enable cancels that cleanup through the flag.
- [Native trampoline setter, `src/async_wrap.cc` lines 359–366](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L359-L366) clears the stored function when its argument is not a function: `env->set_async_hooks_callback_trampoline(Local<Function>());`. It does not validate a user argument or call user code.
- [Native enqueue, `src/node_task_queue.cc` lines 131–138](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_task_queue.cc#L131-L138): `CHECK(args[0]->IsFunction());` is satisfied by the internal cleanup function. [V8 microtask queue lines 81–113](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/microtask-queue.cc#L81-L113) constructs a `NewCallableTask` and enqueues it; it does not execute callbacks synchronously.
- `stopPromiseHook` is set by [internal async hooks lines 382–399](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L382-L399). [Internal promise hooks lines 74–117](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/promise_hooks.js#L74-L117) show its closure only iterates internally generated bound stop functions: `for (const stop of hooks) { stop(); }`. `stop` finds/removes an internal function and calls `update`, which selects existing function references and calls `setPromiseHooks`. It does **not** call the user hooks. Function validation at `makeUseHook` is on setup, not the cleanup path.
- [Native `SetPromiseHooks`, lines 201–208](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L201-L208) forwards function-or-empty handles to [Environment::ResetPromiseHooks, `src/env.cc` lines 83–118](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/env.cc#L83-L118), which updates stored handles and contexts. [V8 Context::SetPromiseHooks, lines 7323–7365](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/api/api.cc#L7323-L7365) sets native-context slots. The compile-time alternative without `V8_ENABLE_JAVASCRIPT_PROMISE_HOOKS` is an API-check fatal failure, not a JS exception; it is not the tested build. No operating-system error path is present here.
- Separate callback delivery is covered by [internal async hooks lines 192–258](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L192-L258), which catches callback errors with `fatalError(e)`. [Lines 160–175](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L160-L175) abort/exit. Those are not ordinary throwable effects of `disable`; the success repro catches assertions *inside* its self-disabling callback to avoid destabilizing delivery.

## Mechanism review and current repros

The registered-hook guard controls reads of `this[init|before|after|destroy|promiseResolve]`
(the actual internal symbols, not string keys). The existing `checkReads` fixture
targets each symbol separately: a normally returning invariant-violating trap
creates TypeError; a throwing trap propagates its sentinel unchanged. Three copies
of that propagation matrix with different sentinel types have been reduced to one
`propagates supplied effect` case. Unregistered/previously disabled receivers and
reentrant disable controls remain, because they prove effects are avoided by state.
These are receiver/state dependencies, not invocation of the hook callbacks.
Current execution count: 3; latest validation is in the run checkpoint.

## Original audit validation (historical)

All five tests have `{ tags: ["node-only"] }`. They run real built-ins in synchronous child processes started with `process.execPath`, so hooks never touch Vitest's async bookkeeping. Each child asserts locally in its own realm and emits a marker with its actual Node version; the parent checks both successful exit and the marker matching its runtime. No `.rejects` is appropriate: errors are synchronously caught inside each child. Ten-second child timeout bounds fixtures. Each enabled fixture is disabled in `finally`; failure switches/traps are restored first. Zero-callback fixtures keep counters zero while exercising all five reads. No active revoked hook is left behind.

| Test | Branch and assertions |
| --- | --- |
| `throws TypeError` | Proxy invariant violation on each of the five frozen target fields, plus non-callable handler `get`. Exact `constructor === TypeError`, `name === "TypeError"`, no Node `code`. |
| `throws Error` | Supplied Error propagated from each symbol read; strict identity and exact constructor. |
| `throws SyntaxError` | Independently supplied SyntaxError propagated from each symbol read; strict identity and exact constructor. |
| `throws non-Error value` | Supplied frozen object and `undefined`, each on all five reads; strict identity and explicit caught flag. |
| `returns the receiver without brand checks` | Unregistered nullish/primitive/object/revoked receivers; inactive, enabled, and repeatedly disabled genuine hook; last-hook cleanup; self-disabling inside an init callback exercises temporary hook-array copying. |

Executed from repository root, with Node v24.19.0 / V8 13.6.233.17-node.51 and Vitest 5.0.5:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/async_hooks.test.ts \
  -t '^node:async_hooks > AsyncHook > disable > (throws |returns )' --maxWorkers 1
# Final validation after adding the non-callable-get branch:
mise run //pkgs/lib:test/node tests/node/24/async_hooks.test.ts \
  -t '^node:async_hooks > AsyncHook > disable > (throws |returns )' --maxWorkers 1 --reporter verbose
```

Both runs: **1 file passed, 5 tests passed, 25 unrelated TODOs skipped**. Final verbose output explicitly identified all five repros in **`node-24`**. The full suite path has no regex metacharacters needing escaping; `>` boundaries prevent matching `AsyncLocalStorage.disable` or other callables. Chromium is excluded by the Node inventory/project mapping and was not run. No inventory generation, shared configuration, dependency, helper, manifest, or declaration changes were made.

## Open questions

No unresolved behavioral paths within the stated scope. Annotation delivery remains a coordinator concern: external `@types/node` declarations currently lack a durable annotation transform. This audit proposes a receiver-effect-aware contract (conservative fallback `unknown`) only; repro success does not install it. No runner/configuration change is required.
