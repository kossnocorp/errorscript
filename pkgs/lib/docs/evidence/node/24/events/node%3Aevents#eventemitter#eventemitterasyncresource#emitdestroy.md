# node:events > EventEmitter > EventEmitterAsyncResource > emitDestroy

- ID: `node/24/events.test.ts::["node:events","EventEmitter","EventEmitterAsyncResource","emitDestroy"]`
- Test: `pkgs/lib/tests/node/24/events.test.ts`, exact managed path above.
- Declaration: `@types/node@24.19.2/events.d.ts:519–543`, `EventEmitter.EventEmitterAsyncResource.emitDestroy(): void`; one signature, no overloads. `events` exports `EventEmitter` at line 962; `node:events` aliases it at 964–966. Declaration SHA-256: `1049c567c0a6244b69738cdd83edce170b8d5ca941817cf185f7f55c28b3feb0`.
- Runtime mapping: `EventEmitterAsyncResource.prototype.emitDestroy` in `lib/events.js`, delegating to inherited `AsyncResource.prototype.emitDestroy` on a private, genuine `EventEmitterReferencingAsyncResource` instance. The similarly named interface is not a separate runtime method.
- Status: complete (behavior/evidence; no declaration annotation delivered).
- Contract candidate: `Returns<void, TypeError | ResourceStateEffects>`, where `ResourceStateEffects` denotes arbitrary thrown values from the exposed resource's property access, assignment, and coercion. Conservative fallback: **`Returns<void, unknown>`**. With intact resource data/prototypes, `Returns<void, TypeError>` covers invalid receivers; for a genuine receiver with intact resource state, the synchronous error component is `never`. Destroy-hook failures are a separate fatal delivery effect, not an error component of this return contract. No error-class argument to `Throws` is proposed.

## Conclusion

**Synchronous intrinsic type: `TypeError`.** Missing private brand (including nullish, primitive, prototype-only, proxy-wrapped, or revoked-proxy receivers) fails before delegation. The receiver proxy's `get` trap is not invoked. With adversarial state in the publicly exposed `asyncResource`, strict writes to a non-writable/getter-only/frozen `destroyed` property, nullish/primitive state, proxy failures, or invalid async-ID coercion can also create `TypeError`.

**Synchronous propagation: any JS value.** The private resource itself is not replaceable, but its ordinary symbol properties are mutable/reflectable. Both reads of its destroyed-state symbol, the `destroyed` assignment, the async-ID symbol read, and (with destroy hooks enabled) numeric comparison can execute supplied getters/setters/proxy traps/conversion functions. There is no catch on this chain. `Error` and `SyntaxError` repros prove identity propagation, not intrinsic creation. Built-in methods/prototypes remain unmodified in all repros.

**Deferred delivery:** enabled destroy hooks run from a native immediate (or, for a large queue, a queued microtask). A throwing hook is caught by Node's hook dispatcher and normally terminates the process with status 1; `--abort-on-uncaught-exception` instead requests abort. This is not a synchronous throw, promise rejection, iteration error, or EventEmitter `error` event from `emitDestroy`. No promise or iterator is returned; return value is `undefined`. Extra arguments are ignored, with no conversion. The implementation does **not** reject a second call: it queues another notification while hooks are enabled, including when the automatic-destruction flag is already true. This contradicts the declaration comment at lines 525–528; that comment is not proof of an intrinsic `Error`.

Scope: Node v24.19.0 on Linux; intact built-in implementations and host intrinsics, including the delegated `emitDestroy` method. Includes invalid receivers and deliberately changed resource data/accessors; excludes replacing built-in functions, native-addon corruption, resource/stack exhaustion, and process termination from the throwable contract. Corrupted non-number async IDs that pass `> 0` reach a native fatal `CHECK`, not a JS exception; this is explicitly distinguished below. Construction/init callbacks occur before the audited call and are not attributed to it. Automatic GC notification is separate from the explicit call.

## Proof

### Exact versions

Node clone HEAD: `510d84c330e8e7d2bb8699608163024baf54eaa6` (not used for runtime proof). All Node/V8 links use **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, tag `v24.19.0`. [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines 24/19/0. Actual executable under `mise`: **v24.19.0**, **V8 13.6.233.17-node.51**, matching the vendored V8 version. The ambient shell's Node was v22.23.1, so it was not used for validation.

Additionally compared `process.binding('natives')[name]` byte-for-byte to `git show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/<name>.js`; all returned `true`:

| Module | SHA-256 |
| --- | --- |
| `events` | `00b82a669fd6299ea5f3916473eb922a74018f96612b290024fffe399580f2c1` |
| `async_hooks` | `fa88748e8443544189029826955f0cdb7386473a0b7191956d7529a22457dff7` |
| `internal/async_hooks` | `08a5d05e584c48a6f51edd61e1e6cb117906d763e521e4b8dc2c7227462bbdca` |

ECMA-262 clone HEAD/revision: **`40ce6f206b5669e69bf010a38267ae24a2843404`**. All spec algorithm anchors below were read in that [pinned spec source](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html). Engine proof uses matching Node-vendored V8, not unrelated Chromium HEAD. Chromium is not a runtime for this Node inventory entry.

### Dispatch and private receiver access

1. [lib/events.js:103–158, 175–177, 196–198, 212–244](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L103-L244): lazy export defines `class EventEmitterReferencingAsyncResource extends AsyncResource`, and the emitter constructor assigns `this.#asyncResource = new EventEmitterReferencingAsyncResource(this, name, options)`. The selected body is exactly `this.#asyncResource.emitDestroy();`; there is no argument processing, catch, or explicit return. The public `asyncResource` getter returns the same private object. Namespace and named-import paths resolve to this class.
2. [GetValue](https://tc39.es/ecma262/#sec-getvalue), spec lines 4472–4496, performs `? ToObject` and then `? PrivateGet` for a private reference. [PrivateGet](https://tc39.es/ecma262/#sec-privateget), lines 6795–6818: “If _entry_ is ~empty~, throw a *TypeError* exception.” [PrivateElementFind](https://tc39.es/ecma262/#sec-privateelementfind), lines 6717–6731, searches the receiver's own `[[PrivateElements]]`, not its prototype or proxy target. Thus forwarding proxies cannot supply the private slot; there is no user trap at this step. Null/undefined fail ToObject, while other primitives cannot hold this brand. Matching [V8 objects.cc:1299–1317](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/objects.cc#L1299-L1317) creates `NewTypeError(MessageTemplate::kInvalidPrivateMemberRead, ...)` on missing private fields.
3. The property lookup/call of the resource's `emitDestroy` is ordinary. Under intact built-in methods it resolves to the inherited `AsyncResource` method. Replacing that property/prototype can of course introduce arbitrary effects or a non-callable TypeError, but is not used to prove built-in behavior here.

### Delegated reads, writes, conversions, and all normal synchronous branches

4. [lib/async_hooks.js:177–217, 235–241](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/async_hooks.js#L177-L241): constructor creates numeric `this[async_id_symbol]`; when `!requireManualDestroy && destroyHooksExist()`, it also creates a plain `{ destroyed: false }` state bag and registers a GC hook. Delegated method:

   ```js
   if (this[destroyedSymbol] !== undefined) {
     this[destroyedSymbol].destroyed = true;
   }
   emitDestroy(this[async_id_symbol]);
   return this;
   ```

   The outer emitter method discards that return. With absent destroyed state the assignment is skipped; with ordinary state it sets the flag. There is **no test rejecting an already-true flag**. No ordinary Error/SyntaxError constructor or validation error is present. Freezing the emitter does not freeze the separate state bag.
5. [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget), spec lines 13337–13361, ends accessor access with `Return ? Call(_getter_, _receiver_)`. Therefore the two distinct destroyed-symbol reads and the async-ID read can each propagate arbitrary values. Missing own properties can similarly reach inherited getters/proxies. Comparison with `undefined` is strict and does not itself coerce.
6. Assignment runs in a strict class method. [PutValue](https://tc39.es/ecma262/#sec-putvalue), lines 4499–4530: “If _succeeded_ is *false* and _refRecord_.[[Strict]] is *true*, throw a *TypeError* exception.” [OrdinarySetWithOwnDescriptor](https://tc39.es/ecma262/#sec-ordinarysetwithowndescriptor), lines 13397–13434, returns false for non-writable fields, non-object receivers, or absent setters; otherwise `Perform ? Call(_setter_, _receiver_, « _value_ »)`. Nullish state fails property-reference conversion. Thus assignment can intrinsically fail or propagate a supplied setter failure. [Proxy [[Set]]](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-set-p-v-receiver), lines 16333–16364, uses `? ValidateNonRevokedProxy`, `? GetMethod`, `? Call`, target descriptor reads and invariant TypeErrors. A false `set` result reaches the strict-write TypeError. Revocation/non-callable traps/invariants add the same intrinsic class; trap lookup/call/target operations can propagate arbitrary values. These statements also cover proxy participation in inherited ordinary reads and conversion lookup; no finite class list captures supplied code.
7. [lib/internal/async_hooks.js:478–495, 533–538](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L478-L538): `hasHooks` only reads a native numeric counter. `if (!hasHooks(kDestroy) || !(asyncId > 0)) return;` short-circuits before comparison if there are no destroy hooks. The async-ID **property read already occurred** in the caller even on this branch. Nonpositive/NaN IDs return normally; a normal positive numeric ID proceeds to `async_wrap.queueDestroyAsyncId(asyncId)`.
8. [IsLessThan](https://tc39.es/ecma262/#sec-islessthan), lines 6034–6093, implements the `>` comparison with `? ToPrimitive(..., number)` and `? ToNumeric`; no conversion is applied to the callable's unused arguments. [ToPrimitive](https://tc39.es/ecma262/#sec-toprimitive), lines 5027–5055, calls a supplied conversion method and says “Throw a *TypeError* exception” if it returns an object. GetMethod's non-callable method and OrdinaryToPrimitive's failure to produce a primitive also give TypeError; getters or conversion callbacks propagate any thrown value. [ToNumber](https://tc39.es/ecma262/#sec-tonumber), lines 5120–5142, rejects Symbol. ToNumeric preserves BigInt; a positive corrupted BigInt/string/object ID does **not** get converted in place to a number for the following native call.

### Native queue, delivery, and fatal-only paths

9. [src/async_wrap.cc:352–357](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L352-L357): `CHECK(args[0]->IsNumber());`, then `args[0].As<Number>()->Value()`. It neither coerces arbitrary input nor throws a recoverable JS validation error. The ordinary generated ID satisfies this assertion. Deliberately corrupting the exposed ID to a positive non-number reaches a fatal native assertion, outside the throwable contract, not an unreported Error class.
10. [src/async_wrap.cc:579–603](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L579-L603): returns if no destroy hooks or `!env->can_call_into_js()`. Otherwise an empty queue schedules `env->SetImmediate(&DestroyAsyncIdsCallback, CallbackFlags::kUnrefed)`, and the ID is appended. At queue size 16384, `RequestInterrupt` enqueues a microtask calling the same callback; it does not invoke a user hook as part of `emitDestroy`'s JS call. [src/env-inl.h:574–608](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/env-inl.h#L574-L608) pushes native callbacks onto queues; the unrefed case does not toggle ref state. [src/env.cc:1446–1477](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/env.cc#L1446-L1477) requests a V8 interrupt, guards an already-scheduled interrupt and destroyed Environment, then drains internal interrupt callbacks. No ordinary JS exception-producing validation or OS-error branch is introduced. Allocation failure/process teardown are outside normal throwable effects.
11. [src/async_wrap.cc:68–89](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L68-L89) drains IDs and calls `env->async_hooks_destroy_function()`. [lib/internal/async_hooks.js:109, 229–268](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L229-L268) maps this to `emitHook` bound to the destroy symbol. Its loop invokes `active_hooks.array[i][symbol](asyncId)` under `catch (e) { fatalError(e); }`. This also catches throwing hook-property reads; hook additions/removals are restored after delivery. [fatalError:155–175](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/async_hooks.js#L155-L175) prints the stack/inspected value, checks `--abort-on-uncaught-exception`, then `process.abort()` or `process.exit(kGenericUserError)`. Even effects from inspecting a hostile thrown value occur in this deferred fatal path, not the initiating call. The callback return value is ignored; returned/rejected user promises are not awaited by this callable.
12. [src/async_wrap.cc:229–267](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/async_wrap.cc#L229-L267) registers the optional weak GC callback and reads the original bag's `destroyed` flag later, queuing notification if empty/false. The explicit call's flag write suppresses that automatic notification for intact state. Weak-callback execution, GC timing, and its property-access effects are not synchronous effects of the explicit call. No platform-specific alternative in this call chain changes the ordinary JS error set; the abort option changes fatal delivery only.

## Repros and validation

All six tests carry `{ tags: ["node-only"] }` and execute real built-ins in isolated children launched with `process.execPath`. No mocks, polyfills, method replacement, or shared helper changes. Ten-second timeouts bound child processes; normal children assert exact constructors and supplied-value identity in their own realm and report the actual runtime version. Resource descriptors/data are restored in `finally` before cleanup; enabled hooks are disabled. Fatal-delivery children intentionally exit and cannot contaminate the runner; they disable the test hook before throwing. No `.rejects` assertions: the four error cases are synchronous within the child, and the fatal cases are neither promises nor callable throws.

| Test | Coverage |
| --- | --- |
| `throws TypeError` | Nullish/primitive/unbranded/prototype-only/proxy/revoked receivers; proxy get trap remains uncalled. Null/primitive/frozen/getter-only/proxy-false destroyed bags; Symbol and object-returning conversion failures. Every caught error has exact `constructor === TypeError`, `name === 'TypeError'`, and no Node code. |
| `throws Error` | Independently supplied Error, exact identity and constructor, across first/second destroyed read, setter, async-ID read, conversion callback. |
| `throws SyntaxError` | Independently supplied SyntaxError, same five branches, exact identity and constructor. |
| `throws non-Error value` | Frozen object and `undefined`, same five branches, strict identity plus caught flag. |
| `returns before destroy delivery, including repeated calls` | No-hook path; manual/automatic state paths with hooks; repeated calls return undefined, immediate callback count stays zero, later each ID appears twice. Automatic destroyed flag becomes true. |
| `delivers throwing hooks fatally, not as synchronous throws` | Three fresh children: hook throws Error, SyntaxError, or string. Ordered `returned`/`delivered` markers, actual version, exit 1, no signal, sentinel diagnostic, and no `uncaughtException` event. This tests fatal delivery, not identity propagation to a caller. |

Executed from repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/events.test.ts \
  -t '^node:events > EventEmitter > EventEmitterAsyncResource > emitDestroy > (throws |returns |delivers )' \
  --maxWorkers 1 --reporter verbose
```

Result: **1 file passed, 6 tests passed, 41 unrelated TODOs skipped (47 total)**, all six explicitly listed under **`node-24`**. Runtime **Node v24.19.0 / V8 13.6.233.17-node.51**, Vitest **5.0.5**. The full callable filter has no regex metacharacters needing escaping and includes the trailing `>` boundary. Chromium excluded by Node inventory mapping and not run. Source comparison was executed using `mise exec -- node`, `execFileSync('git', ['-C', '<clone>', 'show', '<revision>:lib/<module>.js'])`, strict equality with embedded natives, and SHA-256 as recorded above. No inventory generation/checkpoint, manifest, shared runner/configuration, dependency, or declaration edits.

## Open questions

No unresolved behavior within the stated scope. Node annotation delivery is deferred to the coordinator; passing repros do not modify declarations. No shared configuration change required. Reviewers should retain the distinction between intact-resource `TypeError`, adversarial resource-state `unknown`, and fatal deferred destroy-hook delivery; the declaration's repeated-call error claim is contradicted by the matching implementation and repro.
