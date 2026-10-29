# node:diagnostics_channel > Channel > bindStore

- ID: `node/24/diagnostics_channel.test.ts::["node:diagnostics_channel","Channel","bindStore"]`
- Test: `pkgs/lib/tests/node/24/diagnostics_channel.test.ts`, exact managed suite above.
- Declaration: `@types/node` **24.19.2**, `diagnostics_channel.d.ts:138,240,574–576`: `Channel<StoreType = unknown, ContextType = StoreType>.bindStore(store: AsyncLocalStorage<StoreType>, transform?: (context: ContextType) => StoreType): void`. One signature, no overloads. `node:diagnostics_channel` re-exports `diagnostics_channel`; this is a concrete method, not the transform callback's implementation.
- Runtime mapping: exported `Channel.prototype.bindStore` activates an inactive channel and dynamically dispatches to `ActiveChannel.prototype.bindStore`. Active instances have the latter prototype, not `Channel.prototype`.
- Status: complete (behavioral audit; annotation delivery deferred).
- Contract candidate: `Returns<void, TypeError | RangeError | ReceiverEffects>`; synchronous intrinsic `TypeError | RangeError` plus arbitrary receiver-code propagation. `ReceiverEffects = unknown` for unrestricted receivers/proxies/accessors; with ordinary unmodified Channel state there is no user-code effect. Do **not** include the supplied transform's thrown values in this call's effect: it is merely stored.

## Conclusion

All established effects are **synchronous**:

- **TypeError**: null/undefined receivers; primitive receivers when activation writes properties in strict mode; non-extensible inactive receivers whose prototype cannot change; read-only activation fields; active receivers with missing/non-callable `_stores.has`/`set` or invalid Map receivers; revoked proxies and proxy invariant violations.
- **RangeError**: the internal SafeMap reaches V8's collection capacity and inserting another distinct store fails. This is a catchable algorithmic size limit, not an OOM/process-termination claim. In the tested build the boundary is 16,777,216 entries.
- **Arbitrary thrown values**: receiver prototype-change/get/set traps, accessors, dynamically dispatched methods on exotic receivers, and property-key conversion of an exotic `_index` propagate without a catch. The finite intrinsic list is not exhaustive for supplied executable receiver behavior.
- No promise rejection, iteration-time error, callback delivery, or error event is initiated by `bindStore`. It neither validates nor invokes `store`/`transform`, even if they are invalid values or throwing proxies/functions. Replacement of a store is a distinct path that skips reference/count increments.

Scope: Node 24.19.0 and its bundled V8; stock built-ins/primordials, no global monkey-patching, no process termination/OOM. Invalid receivers are deliberately covered. Receiver getters may execute arbitrary code, including other failing APIs; those errors remain propagation rather than additional intrinsic constructors. No OS-dependent branch exists here. String channel names use the subscriber counter; symbol names skip it.

## Proof

### Revisions and implementation mapping

Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, **not** the tested runtime. All Node citations and inspection used `git show` at **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, exact tag `v24.19.0`. [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines major 24, minor 19, patch 0. Runtime reports V8 `13.6.233.17-node.51`; the cited V8 files are bundled at this same Node revision, not Chromium HEAD.

ECMA-262 local HEAD: **`40ce6f206b5669e69bf010a38267ae24a2843404`**. Its language algorithms below corroborate the matching V8 implementation; it is not used as a claim that this newer specification is the runtime's release revision.

### Inactive path and receiver effects

[lib/diagnostics_channel.js:70–75,217–220](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L70-L75): `"ObjectSetPrototypeOf(channel, ActiveChannel.prototype);"`, `"channel._subscribers = [];"`, `"channel._stores = new SafeMap();"`; the inactive method performs `"markActive(this);"` then `"this.bindStore(store, transform);"`.

There is no Channel brand check. Prototype mutation, strict property assignments (file begins `'use strict'`), and the dynamic method lookup/call are observable to an exotic receiver and are not caught. A non-extensible ordinary inactive Channel rejects the different prototype; a read-only `_subscribers` fails after prototype mutation (the repro restores it).

[V8 object.tq:154–172](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/object.tq#L154-L172) uses `"RequireObjectCoercible(object, 'Object.setPrototypeOf');"` and `"return object::ObjectSetPrototypeOfThrow(object, proto);"`. The prototype argument here is always the actual ActiveChannel prototype, so invalid-prototype input is not a reachable argument-validation branch.

[ECMA Object.setPrototypeOf](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L31452-L31462), anchor `sec-object.setprototypeof`: `"If _status_ is *false*, throw a *TypeError* exception."` Its `? RequireObjectCoercible` rejects null/undefined; primitives otherwise pass through and fail on the subsequent strict writes. [PutValue](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L4499-L4526), anchor `sec-putvalue`, says `"If _succeeded_ is *false* and _refRecord_.[[Strict]] is *true*, throw a *TypeError* exception."` This callable has resolved property references, so PutValue's unresolvable-reference ReferenceError branch is not reachable.

[Proxy [[SetPrototypeOf]]](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16020-L16044), anchor `sec-proxy-object-internal-methods-and-internal-slots-setprototypeof-v`, performs `"? ValidateNonRevokedProxy(_obj_)"` and `"ToBoolean(? Call(_trap_, _handler_, « _target_, _proto_ »))"`; non-extensible-target invariant failure explicitly throws TypeError. Abrupt trap completions propagate. The other ordinary property operations likewise propagate accessors/proxy traps, rather than converting them to Node errors.

### Active path, replacement, and counters

[lib/diagnostics_channel.js:133–140](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L133-L140): `"const replacing = this._stores.has(store);"`; only if not replacing, `"channels.incRef(this.name);"` and `"if (this._index !== undefined) subscriberCounts[this._index]++;"`; finally `"this._stores.set(store, transform);"`.

Thus argument objects are opaque keys/values; there is no ToString, inspection, property access, or callback invocation on either argument. The ordinary `_stores` is a SafeMap: [primordials.js:399–450](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/per_context/primordials.js#L399-L450) copies native methods and freezes the safe prototype, constructing `"class SafeMap extends Map {}"`. A new empty Map has no iterable or user callbacks. Invalid active receivers can fail at property reads, calls, or the Map internal-slot check. Valid stores do not need to be AsyncLocalStorage instances at this stage.

[ECMA Map.has and Map.set](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L44309-L44344), anchors `sec-map.prototype.has` and `sec-map.prototype.set`, use `"Perform ? RequireInternalSlot(_map_, [[MapData]])."` Keys are canonicalized without executing user coercion, then compared with SameValue. [Matching V8 Map.has:1603–1620](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/builtins-collections-gen.cc#L1603-L1620) uses `"ThrowIfNotInstanceType(context, receiver, JS_MAP_TYPE, \"Map.prototype.has\");"`; Map.set has the equivalent check at line 1735.

`channels.incRef` is [WeakRefMap.incRef:61–63](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L61-L63): `"return super.get(key)?.incRef();"`. It looks up the name by identity (no coercion); absence skips the call. The internal WeakReference's [incRef:874–883](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util.js#L874-L883) increments a numeric private counter, dereferences its genuine SafeWeakRef on transition to one, and saves that value. No external callback is invoked, and the private receiver is the actual internally-created WeakReference.

[src/node_diagnostics_channel.h:40](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_diagnostics_channel.h#L40) declares `"AliasedUint32Array subscribers_;"`; [constructor/index creation](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L190-L200) sets `_index` only for strings. Genuine indices are numeric and counter arithmetic wraps in Uint32 storage without throwing. Exotic receivers can instead invoke getters/key conversions here, with arbitrary abrupt completions. Native index allocation occurs in channel construction, not bindStore, so its fatal capacity CHECK is not attributed to this callable.

### Catchable Map-capacity RangeError

[Map.set:1729–1761](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/builtins-collections-gen.cc#L1729-L1761) calls `"CallRuntime(Runtime::kMapGrow, context, receiver);"` when insertion requires growth. [Runtime_MapGrow:85–100](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/runtime/runtime-collections.cc#L85-L100) calls EnsureCapacityForAdding and on failure creates `"NewRangeError(MessageTemplate::kCollectionGrowFailed"` with `"Map"`.

[OrderedHashTable::Allocate:19–35 and EnsureCapacityForAdding:77–99](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/ordered-hash-table.cc#L19-L35): `"if (capacity > MaxCapacity())"` explicitly throws a RangeError before allocating the oversized backing store. Growth doubles capacity unless deleted entries permit compaction; replacement does not require growth. [message-template.h:444](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/common/message-template.h#L444) supplies `"% maximum size exceeded"`. The repro fills a real channel's unmodified SafeMap via bindStore itself; the final bindStore, not fixture construction, throws. The child asserts exact RangeError constructor, message, and absence of Node `code`.

### Deferred transform behavior is a different call

[runStores:174–186 and wrapStoreRun:91–104](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L91-L104) show where the saved transform is eventually invoked: `"context = transform(data);"`, with a caught failure scheduled via `"process.nextTick"`/`"triggerUncaughtException(err, false)"`; `"return store.run(context, next);"` follows for successful transforms. Those paths require a **later runStores call**, are not reachable during bindStore, and must not be mislabeled as bindStore rejections/throws or callback execution. No runStores audit is claimed here.

## Repros and validation

Required project: **node-24 only** (Node declarations are not in Chromium's include set). Every repro is tagged `node-only`. All concrete constructor checks use identity, not a base-class match; propagation checks assert the sentinel object's identity.

1. `throws TypeError`: six nullish/primitive inactive receivers; exact TypeError, no Node code.
2. `non-extensible inactive receiver > throws TypeError`: denied prototype activation; remains inactive.
3. `read-only activation property > throws TypeError`: strict assignment fails; descriptor/prototype restored in finally.
4. `active method incompatible receiver > throws TypeError`: nullish, missing/non-callable map methods, and borrowed native Map.has on a non-Map; real bound store unbound and disabled in finally.
5. `activation proxy trap > throws non-Error value`: setPrototypeOf trap sentinel identity; target unmodified.
6. `active receiver property access > throws non-Error value`: `_stores` and `name` access traps; identity, stores unbound/disabled in finally.
7. `throws RangeError`: genuine capacity insertion in an isolated child using the same `process.execPath`, `--max-old-space-size=2048`. Child synchronously proves the precise RangeError; async parent waits only for process completion, **not** a bindStore promise. All successful entries are unbound in finally. The failed insertion increments the internal reference count before failing to insert; child isolation releases that residual internal reference on process exit. Runtime approximately 6 seconds, substantial heap (allow up to 2 GiB); no shared runner configuration changed.
8. `stores arguments without invoking or validating them`: opaque proxy store/throwing transform, replacement by null, and undefined store/numeric transform all accepted; string-name counter branch; all keys unbound in finally and channel verified inactive.

Commands run from repository root with `PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`:

```sh
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/diagnostics_channel.test.ts \
  -t '^node:diagnostics_channel > Channel > bindStore > ' --maxWorkers 1
mise run //pkgs/lib:test/node tests/node/24/diagnostics_channel.test.ts \
  -t '^node:diagnostics_channel > Channel > bindStore > ' --maxWorkers 1 --reporter verbose
```

The full-suite regexp has no metacharacters requiring escaping and ends at the callable's ` > ` boundary, including its nested scenarios only. Initial run before the capacity case: **7 passed, 21 unrelated TODOs skipped**. Final verbose run: **8 passed, 21 unrelated TODOs skipped, one file passed**, every selected case explicitly reported under `node-24`; 6.11 seconds total. Vitest **5.0.5**, Node **24.19.0**, V8 **13.6.233.17-node.51**. Bare shell `node --version` was **22.23.1**, so it was not used for validation; mise selected the verified required runtime.

## Open questions

No unresolved behavioral path or runtime blocker. External Node annotation delivery remains coordinator-deferred; this audit changes no declaration annotations. The resource-heavy capacity repro is isolated and locally timeout-bounded; future constrained CI may need coordinator-managed scheduling/resources, not a mock or a skipped claim of validation.
