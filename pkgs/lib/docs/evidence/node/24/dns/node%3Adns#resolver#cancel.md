# node:dns > Resolver > cancel

- ID: `node/24/dns.test.ts::["node:dns","Resolver","cancel"]`
- Test: `pkgs/lib/tests/node/24/dns.test.ts`, selected managed suite only.
- Declaration: `pkgs/lib/node_modules/@types/node/dns.d.ts:877–884`, `Resolver.cancel(): void`; one signature, no overloads. Installed `@types/node` **24.19.2**. Lines 921–923 re-export `dns` from `node:dns`; this is the callback Resolver, not the separate promises Resolver declaration.
- Runtime mapping: exported callback `Resolver` inherits `ResolverBase.cancel`, which calls `this._handle.cancel()` → native `cares_wrap::Cancel` → `ares_cancel`.
- Status: **complete** (evidence and runtime validation; annotation delivery deferred).
- Contract candidate: `Returns<void, TypeError | E_receiver>`, where `E_receiver` is the effect of the two property gets and dispatch on a supplied receiver. Conservatively `Returns<void, unknown>` for unrestricted receivers. With an intact native Resolver and ordinary built-ins, the synchronous error set is empty. **Do not include callback-delivered `ECANCELLED` as a synchronous `Error` throw.**

## Conclusion

Synchronous intrinsic errors are **TypeError**: nullish receiver/handle, missing or noncallable `cancel`, revoked/invalid proxy operations, and incompatible receiver of the native handle method. These have no Node error code. Native incompatible-receiver validation reports `Illegal invocation`.

The implementation performs ordinary property reads, with no catch. Supplied accessors or proxy traps can throw **any value**, including Error and SyntaxError; those two tests prove propagation by identity, not intrinsic construction. No argument is inspected or converted (extra arguments are ignored). No promise or iterator is returned.

For real pending queries, cancellation creates a **later error-first callback delivery** of an Error-family `DNSException`, exposed with `constructor === Error`, `name === "Error"`, `code === "ECANCELLED"`, and the query's `syscall`/`hostname`. This is not a throw from `cancel`. A callback can itself throw any value later; the child-process repro verifies identity and that `cancel` already returned. No error event is emitted by Resolver. Idle and repeated cancellation return undefined. Previously completed/queued queries are no longer in the native pending list; cancelling cannot retroactively change their outcomes.

Scope: Node 24.19.0 on Linux, V8 13.6.233.17-node.51, c-ares 1.34.6; standard native handles, no internal handle destruction/forgery, no monkey-patched built-ins or user async hooks. OOM, engine termination, and corrupted native state are excluded per audit policy. The native null-channel/OOM branches do not report JavaScript errors. The promises Resolver is a different declaration and is not audited here; cancellation errors associated with its requests belong to those requests' rejection channel, not a promise returned by this method.

## Proof

### Revisions and mapping

All Node links below are pinned to **cdc1b38d40cb567b7ad0b39c86addf830a0af0ae**, the release commit `2026-08-03, Version 24.19.0 'Krypton' (LTS)`. Research used `git show` against `~/code/clones/node`; that clone's HEAD was **510d84c330e8e7d2bb8699608163024baf54eaa6**, deliberately not used as the implementation version. [node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27), [V8 version header](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/include/v8-version.h#L11-L14), and [c-ares version header](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/include/ares_version.h#L33-L36) match the runtime version components. The bundled V8 source is used instead of a mismatched Chromium checkout.

- [lib/dns.js:52–54, 323–327](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/dns.js#L52-L54) imports/exports the callback Resolver; [callback_resolver.js:89–93](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/dns/callback_resolver.js#L89-L93) says `const { Resolver, resolveMap } = createResolverClass(resolver);`.
- [utils.js:306–324](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/dns/utils.js#L306-L324): `class Resolver extends ResolverBase {}`. There is no overriding cancel method.
- [utils.js:90–97](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/dns/utils.js#L90-L97): initialization assigns `this._handle = new ChannelWrap(timeout, tries, maxTimeout);`; the entire cancel body is `this._handle.cancel();`. No validation of arguments, conversion, catch, or return expression follows. Constructor/setup errors are not errors of cancel.

### JavaScript property access and receiver errors

ECMA-262 local revision **40ce6f206b5669e69bf010a38267ae24a2843404** (`~/code/clones/ecma262`, `spec.html`), with permanent [source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html):

- [GetValue](https://tc39.es/ecma262/#sec-getvalue), source lines 4472–4493: `Let _baseObj_ be ? ToObject(_refRecord_.[[Base]]).` and `Return ? ... _baseObj_.[[Get]] ...`. [ToObject](https://tc39.es/ecma262/#sec-toobject), lines 5679–5697: `If _arg_ is either *undefined* or *null*, throw a *TypeError* exception.` These explain nullish receiver/handle errors; primitive receivers are boxed, then usually lack `_handle`.
- [EvaluateCall](https://tc39.es/ecma262/#sec-evaluatecall), lines 19865–19892: `If IsCallable(_func_) is *false*, throw a *TypeError* exception.` This applies to the inner `cancel` dispatch, not fixture evaluation. It ends `Return ? Call(_func_, _thisValue_, _argList_).`
- [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget), lines 13337–13360: `Return ? Call(_getter_, _receiver_).` Thus both `_handle` and `cancel` accessors propagate supplied throws unchanged.
- [Proxy [[Get]]](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver), lines 16292–16318: `Perform ? ValidateNonRevokedProxy(_obj_).`, `Let _trapResult_ be ? Call(_trap_, _handler_, ...)`, and invariant violations `throw a *TypeError* exception.` [ValidateNonRevokedProxy](https://tc39.es/ecma262/#sec-validatenonrevokedproxy), lines 16551–16564: `If _proxy_.[[ProxyTarget]] is *null*, throw a *TypeError* exception.` Trap/getter exceptions and invariant checks are reached inside cancel's own property access. The literal property keys require no user coercion.

### Native signature and cancellation branches

- [cares_wrap.cc:2343–2346](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.cc#L2343-L2346) registers `SetProtoMethod(isolate, channel_wrap, "cancel", Cancel);`. [util.cc:627–637](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util.cc#L627-L637) installs `Signature::New(isolate, that)`. Matching [V8 builtins-api.cc:93–96](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/builtins-api.cc#L93-L96) rejects incompatible receivers with `NewTypeError(MessageTemplate::kIllegalInvocation)`. The repro passes the real native method a wrong receiver in a subprocess; no binding is mocked and no native abort occurs.
- [cares_wrap.cc:2251–2259](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.cc#L2251-L2259): `ASSIGN_OR_RETURN_UNWRAP(&channel, args.This());` then `ares_cancel(channel->cares_channel());`. [base_object.h:249–254](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/base_object.h#L249-L254) returns if the unwrapped pointer is null. There is no status-to-error conversion here, unlike other DNS methods; `ERR_DNS_SET_SERVERS_FAILED`, initialization failures and query-start errors are not cancel errors.
- [ares_cancel.c:34–87](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/src/lib/ares_cancel.c#L34-L87): null channel returns; nonempty pending list is swapped so only existing requests are cancelled; failed list allocation restores the list and returns without reporting an error (`this function doesn't return a result code though so we can't report to caller`). Each existing query gets `query->callback(query->arg, ARES_ECANCELLED, 0, NULL);`, then is freed. Empty lists skip callback dispatch. Cleanup and unlock happen afterward. This is native callback invocation, not immediate JS callback invocation.
- Cleanup follows [ares_close_sockets.c:44–70, 83–137](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/src/lib/ares_close_sockets.c#L44-L137): connections with pending queries are skipped; eligible empty connections close and notify socket-state callbacks. Node installs its own native callback in [cares_wrap.cc:906–914](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.cc#L906-L914); its close branch [125–167](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.cc#L125-L167) closes poll handles/timer, not JS user code. [ares_threads.c:601–614](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/src/lib/util/ares_threads.c#L601-L614) only broadcasts the empty-queue condition. Native thread/socket platform choices do not introduce an additional JS exception path.

### Deferred query completion and callback propagation

- Normal DNS record queries preserve unsuccessful status in [ares_query.c:39–58](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/src/lib/ares_query.c#L39-L58): `qquery->callback(qquery->arg, status, timeouts, dnsrec);`. Reverse-query handling [ares_gethostbyaddr.c:158–192](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/cares/src/lib/ares_gethostbyaddr.c#L158-L192) explicitly treats `ARES_ECANCELLED` like destruction and calls `end_aquery(aquery, status, NULL)` rather than falling back to another lookup. Both native callback forms are exercised.
- [cares_wrap.h:312–370](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.h#L312-L370): both Callback overloads store status, skip success-only response copying for cancellation, and call `QueueResponseCallback(status)`. That uses `env()->SetImmediate` to call `AfterResponse()` and detach; the synchronous bookkeeping only decrements the active count ([cares_wrap.cc:980–983](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.cc#L980-L983)). A destroyed query's null callback pointer returns without delivery.
- [cares_wrap.h:266–293](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.h#L266-L293): `if (status != ARES_SUCCESS) return ParseError(status);`. Thus cancellation bypasses all record-parsing paths. `ParseError` uses `ToErrorCodeString` and `MakeCallback(...oncomplete...)`; [lines 44–55](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/cares_wrap.h#L44-L55) map `ARES_ECANCELLED` to `"ECANCELLED"`.
- [callback_resolver.js:39–51, 65–69](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/dns/callback_resolver.js#L39-L69): `this.callback(new DNSException(err, this.bindingName, this.hostname));`. This error is an argument to user code, not thrown. No success/TTL/performance branch runs on cancellation. [errors.js:794–831](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L794-L831) defines `class DNSException extends Error`, assigns `this.code = code`, and explicitly exposes `get ['constructor']() { return Error; }`. The test therefore asserts exact public constructor, name, code, and syscall (not only a base `instanceof Error`).
- If user callback code throws, [api/callback.cc:235–242](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/api/callback.cc#L235-L242) preserves failure of `callback->Call`; [env.cc:1402–1424](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/env.cc#L1402-L1424) catches exceptions from native immediates and calls `errors::TriggerUncaughtException`. This is later uncaught callback delivery, with arbitrary-value identity preserved, not a synchronous cancel throw or returned-promise rejection.

## Mechanism review and current repros

The receiver-proxy repro now targets `this._handle` explicitly; the nested
`this._handle.cancel` getter remains a different effect path. Callback delivery
and a callback throwing after `cancel` returns remain separate scenarios, not
synchronous error-union members. `ECANCELLED` is retained solely to distinguish
the intended cancellation branch from unrelated DNS/network failure. Current
counts/results are in the run checkpoint; diagnostic payload assertions from the
original run below are historical.

## Original audit validation (historical)

All eight cases are explicitly tagged `node-only`; only the `node-24` project is required for this Node declaration. Chromium does not include `tests/node/**`.

| Case under the selected suite | Channel / result |
| --- | --- |
| `throws TypeError` | Seven invalid receiver/handle shapes, exact TypeError, no code |
| `proxy validation > throws TypeError` | Revoked proxy and get invariant violation, synchronous |
| `native receiver signature > throws TypeError` | Real native method, incompatible handle receiver; subprocess exits 0 with `native-TypeError`, no signal |
| `throws Error` | `_handle` getter supplied Error, synchronous identity and constructor |
| `throws SyntaxError` | `cancel` getter supplied SyntaxError, synchronous identity and constructor |
| `throws non-Error value` | Receiver proxy trap, synchronous sentinel identity |
| `callback delivery (not a synchronous throw) > throws Error` | Idle/repeated cancellation returns undefined; A and reverse requests get ECANCELLED only after return |
| `callback delivery (not a synchronous throw) > throws non-Error value` | Callback throws supplied sentinel, child uncaughtException checks identity after cancel returns; exit 0, no signal |

Local DNS uses a bound ephemeral IPv4 UDP socket at `127.0.0.1`, deliberately sends no responses, and calls cancel in the same turn as initiating requests. No timeout or remote DNS response is needed to trigger the errors. The test drains callback completions and cancels/closes resources in finally. Child processes use the test runner's `process.execPath`, a 5-second safety timeout, and close their resolver/socket; the global uncaughtException handler is isolated from Vitest.

Commands actually run (2026-10-29):

```sh
# Repository root: initial shell probe revealed Node 22.23.1, so do not use it.
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" node -p 'JSON.stringify(process.versions)'
# Explicitly select the requested binary:
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec node@24.19.0 -- node -p 'JSON.stringify(process.versions)'
# cwd: /home/koss/code/errorscript/pkgs/lib
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec node@24.19.0 -- pnpm exec vitest run --project node-24 tests/node/24/dns.test.ts -t '^node:dns > Resolver > cancel > (?:.* > )?throws ' --maxWorkers 1 --reporter verbose
```

Actual runtime: Node **24.19.0**, V8 **13.6.233.17-node.51**, c-ares **1.34.6**, libuv **1.52.1**. Vitest **5.0.5**: **1 file passed, 8 tests passed, 39 unrelated TODO tests skipped (47 total)**. All eight expected repro names appeared as executed, with no unhandled runner errors. No mocks/polyfills, inventory regeneration, shared configuration changes, or declaration edits were used.

## Open questions

No remaining callable-proof/runtime blocker. Node annotation delivery is deferred to the coordinator; this document is a candidate, not an installed annotation. No shared configuration request.
