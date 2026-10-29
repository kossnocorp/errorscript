# node:http2 > ClientHttp2Session > addListener

- ID: `node/24/http2.test.ts::["node:http2","ClientHttp2Session","addListener"]`
- Test: `pkgs/lib/tests/node/24/http2.test.ts`, selected managed suite only.
- Declaration: [`@types/node@24.19.2/http2.d.ts`](https://unpkg.com/@types/node@24.19.2/http2.d.ts),
  SHA-256 `f952e6977043709c36ea7d9f626219f7bcb066b0da6277257eea0d2ec76123ce`.
  `ClientHttp2Session extends Http2Session` (1012), five direct overloads (1065–1080):
  `altsvc(alt, origin, stream)`, `origin(origins)`, `connect(session, socket)`,
  `stream(stream, headers, flags, rawHeaders)`, general `string | symbol` event/variadic
  callback. All return `this`. The base `Http2Session extends EventEmitter` (731)
  declares nine overloads (933–947): `close`, `error`, `frameError`, `goaway`,
  `localSettings`, `ping`, `remoteSettings`, `timeout`, general `string | symbol`.
  `node:http2` re-exports `http2` (2880–2882). The interface is not itself a constructor
  export; the runtime instance comes from `http2.connect`.
  The inherited generic base signature is in
  [`events.d.ts:633`](https://unpkg.com/@types/node@24.19.2/events.d.ts):
  `addListener<K>(eventName: Key<K, T>, listener: Listener1<K, T>): this`.
  Its merged EventEmitter interface extends `NodeJS.EventEmitter<T>` (69); with the
  default event map, Key/Listener1 reduce to `string | symbol` / variadic callback
  (70–85). It adds no separate runtime implementation.
- Runtime mapping: `ClientHttp2Session` → `Http2Session` → `EventEmitter.prototype.addListener`
  → `_addListener(this, type, listener, false)`. There is no HTTP/2 addListener/emit override,
  but there **are** native-field `newListener` hooks, custom inspection, and an inherited
  HTTP/2-specific capture-rejection handler; these are covered below.
- Status: complete (behavior/evidence; declaration annotation delivery deferred).
- Contract candidate: `Returns<this, unknown>`; refined synchronous candidate
  `Returns<this, TypeError | ReferenceError | RangeError | Error | RegistrationEffects>`.
  `RegistrationEffects` includes arbitrary thrown values from supplied code/object
  operations, not just Error subclasses. This is not the effect of invoking the newly
  registered callback. No declaration annotations were edited.

## Conclusion

Synchronous intrinsic errors:

- **TypeError**: invalid listener (`ERR_INVALID_ARG_TYPE` if formatting succeeds),
  invalid/nullish/primitive/frozen receiver, revoked proxies/invariant failures, failed
  property-key conversions, malformed event state, and warning string conversions.
  Borrowing a real session's event hooks onto a receiver without its native fields also
  fails with TypeError for native-tracked events.
- **ReferenceError**: an invalid listener can be a module namespace whose exported
  `constructor` binding is still uninitialized. The diagnostic's `value.constructor`
  read raises ReferenceError *inside addListener*, before the intended TypeError is made.
- **RangeError**: the unbranded method can append to a forged receiver's full sparse
  listener Array (`length = 2**32 - 1`), causing the native Array length check to fail.
  Also, mutually returning custom-inspection hooks make Node's diagnostic/warning
  inspection recurse until the engine raises RangeError; neither supplied hook throws.
- **Error**, `ERR_UNHANDLED_ERROR`: capture-rejections enabled before session creation;
  an existing `newListener` callback returns a thenable whose `then` getter/call throws
  a non-Error; `addCatch` synchronously emits `error` without a handler and wraps it.
- **Error**, `ERR_INTERNAL_ASSERTION`: both invalid-listener diagnostic and warning
  inspection can implicitly bypass depth -1 when a custom hook returns a function
  whose `name` is a namespace object. Formatting an exported object's throwing inspect
  hook reaches Node's namespace assertion, replacing the supplied value with this Error.

Arbitrary synchronous effects propagate from `newListener` callbacks, listener `.listener`
access, receiver/event-map getters/setters/proxies, key/max-listener coercions, diagnostic
and inspection hooks, and thenable/error/errorMonitor callbacks. Error and SyntaxError
callback cases assert identity; those exceptions are supplied, not created by registration.
Mutation may precede failure, notably warning formatting and native listener bookkeeping.

No promise or iterator is returned. With capture enabled, a rejected promise from an
existing `newListener` callback schedules the **HTTP/2 capture handler**, which calls
`session.destroy(reason)` for the event `newListener`. This is not EventEmitter's default
error-only handler: the session is destroyed. Truthy nonnumeric reasons are delivered by
identity to the session's subsequent `error` event; a nonzero numeric reason becomes
`Error`/`ERR_HTTP2_SESSION_ERROR`, zero means normal destruction. Undefined uses destroy's
zero default, and null/other falsy values do not cause the final `error` event. A previously
destroyed session makes destroy a no-op. User overrides of the documented rejection-symbol
handler have their own arbitrary callback effects. Unhandled error events or throwing
delivery callbacks can throw asynchronously, not reject addListener. With capture disabled,
returned promises are not observed by registration.

Exceeding the max-listener threshold normally schedules an Error-valued
`MaxListenersExceededWarning`; it does not throw that warning synchronously. Warning
listeners' exceptions belong to later delivery. The callback being registered is not
invoked by registration; network/protocol/error events during later session operation
are not automatically effects of this call.

Scope: Node 24.19.0, standard built-ins, actual HTTP/2 sessions, public capture-rejections
configuration, invalid receivers/arguments and custom object operations. No monkey-patched
built-ins, domains, OOM/process termination, or failures in creating the fixture are used
as callable errors. Plain TCP and TLS clients share these registration paths; TLS setup,
protocol validation and connection failures belong to connect/later I/O, not addListener.
Input-induced recursion within the callable's inspection algorithm is included; an
already-exhausted caller stack is not needed or used. Tests do not alter stack limits.

## Proof

### Version and actual HTTP/2 implementation

Local Node clone HEAD: `510d84c330e8e7d2bb8699608163024baf54eaa6` (not the runtime).
All Node references use **v24.19.0**, revision
`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`, read with `git show` without checkout.
Runtime embedded JS for all eight modules listed under validation matches this revision
byte-for-byte. ECMA-262 clone revision: `40ce6f206b5669e69bf010a38267ae24a2843404`;
the [pinned specification](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html)
supplies the stable JS semantics cited by algorithm anchor below.

- [core.js:3645–3667](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L3645-L3667)
  selects user connection/TCP/TLS, then says `const session = new ClientHttp2Session(options, socket);`.
  [1811–1815](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1811-L1815)
  has `class ClientHttp2Session extends Http2Session`; [1292–1294](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1292-L1294)
  has `class Http2Session extends EventEmitter` and `super()`. Neither class replaces
  registration or emit. Each repro verifies the function identity on a native session.
- [events.js:607–611](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L607-L611)
  says `return _addListener(this, type, listener, false);`. Prepend branches are unreachable.
- [core.js:1359–1361](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1359-L1361)
  creates the tracking typed array when needed and registers
  `this.on('newListener', sessionListenerAdded)`. The [hook:507–524](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L507-L524)
  switches strictly on event names: ping/altsvc/remoteSettings OR numeric bit flags;
  priority/frameError increment numeric byte counters. Other names, including coercible
  objects, do not match this switch. These operations neither validate session liveness
  nor directly call C++/nghttp2. A malformed borrowed receiver fails on its missing field.
- [trackAssignmentsTypedArray:1062–1088](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1062-L1088)
  proxies a Uint8Array, uses `ReflectGet` and `ReflectSet`, and records numeric indices.
  The hook's numeric keys/values require no user conversions on a normal session.
  [setupHandle:1122–1129](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1122-L1129)
  copies assigned entries and switches to `handle.fields`; [cleanup:1175–1192](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1175-L1192)
  restores a new tracking array. Thus pending/connected/destroyed sessions all retain
  usable numeric storage (all three states are exercised).
- [node_http2.cc:618–620](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_http2.cc#L618-L620)
  exports `Uint8Array::New(js_fields_.GetArrayBuffer(), 0, kSessionUint8FieldCount)`.
  [node_http2.h:549–576](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_http2.h#L549-L576)
  defines the valid byte-field offsets and bit positions. Byte counters wrap; they do not
  introduce a RangeError. [TypedArraySetElement](https://tc39.es/ecma262/#sec-typedarraysetelement)
  converts numeric values and writes the buffer; even invalid/detached indices are no-ops,
  not an independent session-state exception. Native socket setup failures are outside this call.

### Validation and ordinary JS operations

- [events.js:265–267,540–599](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L540-L599)
  checks the listener first. [validators.js:471–474](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L471-L474):
  `if (typeof value !== 'function') throw new ERR_INVALID_ARG_TYPE(name, 'Function', value);`.
  [errors.js:1389–1459](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1389-L1459)
  registers TypeError and evaluates `determineSpecificType(actual)`; its assertions concern
  fixed valid internal arguments, not an additional AssertionError branch here.
- [errors.js:995–1042](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1042)
  handles primitives, but for objects evaluates `value.constructor`, `'name' in value.constructor`,
  `value.constructor.name`, or inspect. These operations can fail before the coded error exists.
  The module namespace repro passes a valid namespace object as the argument; it does not
  read the uninitialized export until this diagnostic runs.
  [Module namespace [[Get]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-get-p-receiver)
  says `Return ? _targetEnv_.GetBindingValue(...)`;
  [Module GetBindingValue](https://tc39.es/ecma262/#sec-module-environment-records-getbindingvalue-n-s)
  says “If the binding ... is an uninitialized binding, throw a ReferenceError exception.”
  The self-imported fixture exports lexical `constructor` after the assertion, so this
  is an engine-generated ReferenceError, not a callback throwing a manufactured error.
  [errors.js:464–483](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L464-L483)
  explains coded errors' exposed constructor: `return Base;`. Tests also assert stable codes.
- `_addListener` has no brand check. It reads/writes `_events`, `_eventsCount`, event-keyed
  properties, and later `_maxListeners`. Missing event storage is initialized; otherwise
  `newListener` is emitted before insertion, with `listener.listener ?? listener`, and
  the event storage is reread afterwards. First registration assigns; second builds a
  two-element array; subsequent registrations call `ensureMutableListenerArray` and push.
  No catch wraps these operations. Nullish access fails via
  [GetValue/ToObject](https://tc39.es/ecma262/#sec-getvalue); strict failed writes via
  [PutValue](https://tc39.es/ecma262/#sec-putvalue): “If _succeeded_ is false and
  _refRecord_.[[Strict]] is true, throw a TypeError exception.”
- [ToPropertyKey](https://tc39.es/ecma262/#sec-topropertykey) performs `? ToPrimitive`;
  [ToPrimitive](https://tc39.es/ecma262/#sec-toprimitive)/[OrdinaryToPrimitive](https://tc39.es/ecma262/#sec-ordinarytoprimitive)
  propagate conversion callbacks or throw TypeError when no primitive results.
  [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget) uses `? Call` for getters;
  [Proxy get](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver)
  validates revocation/invariants and propagates trap completions; setters have corresponding
  effects. Hence arbitrary values, not merely the finite intrinsic set, must be retained.
- [events.js:874–903](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L874-L903)
  clones a listener array during active emission (`handler[kEmitting] > 0`), preserves
  `warned`, then assigns the copy. Sizes 2–6 use element reads; other sizes use native slice.
  [slice](https://tc39.es/ecma262/#sec-array.prototype.slice) and
  [ArraySpeciesCreate](https://tc39.es/ecma262/#sec-arrayspeciescreate) expose getter/species/
  constructor effects on hostile state and TypeError for nonconstructible species.
  The reentrant newListener test drives the ordinary copy-on-write branch.
- Native [push](https://tc39.es/ecma262/#sec-array.prototype.push) ends with
  `? Set(_obj_, "length", ...)`. [ArraySetLength](https://tc39.es/ecma262/#sec-arraysetlength)
  throws RangeError when the uint32 length differs from its numeric value. Appending to
  the sparse length-`2**32 - 1` fixture reaches `2**32`; fixture construction itself succeeds.
  This is not a claim that an ordinary session practically accumulates billions of listeners.

### Synchronous callbacks and asynchronous capture delivery

- [events.js:456–537](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L456-L537)
  invokes `handler.apply(this, args)` or `ReflectApply(handler[i], this, args)` without
  catching callback exceptions (array bookkeeping has a finally). Native HTTP/2 already
  installs one newListener handler, so adding a user handler normally exercises the array path.
- [init:332–353](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L332-L353)
  copies the prototype capture setting when no constructor option is provided. Tests use
  the public `EventEmitter.captureRejections` setter before creating each session in a child.
  [addCatch:356–375](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L356-L375)
  does nothing if capture is off; otherwise reads `promise.then`, calls it if callable,
  and catches synchronous failures with `that.emit('error', err)`.
- The synchronous emit first invokes errorMonitor if present, then an error handler if
  present. Without an error handler it rethrows Error instances by identity; for other
  values it tries inspect (catching inspection failure), then creates `ERR_UNHANDLED_ERROR`
  and assigns `err.context = er` (events.js:467–500). Its message template can itself perform
  hostile coercion after a failed inspection. [errors.js:1856–1863](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1856-L1863)
  registers that code with **Error**. This synchronous branch does **not** call the HTTP/2
  rejection-symbol handler; the native then-getter repro verifies it separately.
- A thenable's rejection callback instead queues `process.nextTick(emitUnhandledRejectionOrErr, ...)`.
  [events.js:378–397](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L378-L397)
  prefers `ee[kRejection](err, type, ...args)`. [core.js:1654–1663](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1654-L1663)
  destroys the stream for event `stream`, otherwise `this.destroy(err)`. Here the emitted
  event is **newListener**, even when registering a stream callback, so default is selected.
  A user's custom handler can replace this documented hook; if removed/noncallable, the
  EventEmitter fallback emits error with capture temporarily disabled/restored in finally.
- [destroy:1608–1624](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1608-L1624)
  returns for destroyed sessions, defaults undefined to zero, and converts nonzero numbers
  to `new ERR_HTTP2_SESSION_ERROR(code)`; [errors.js:1302](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1302)
  registers **Error**. Non-null reasons select INTERNAL_ERROR as the code otherwise.
  [closeSession:1227–1254](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1227-L1254)
  marks destroyed, clears timeout/listeners, destroys pending/active streams and the handle.
  Native [Destroy:2936–2947](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_http2.cc#L2936-L2947)
  converts the numeric code to uint32 and closes. Pending streams can receive
  ERR_HTTP2_STREAM_CANCEL; those are stream delivery effects of later destruction, not
  registration throws. Stream/user removal callbacks can also have arbitrary deferred effects.
  [finishSessionClose:1195–1224](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1195-L1224)
  cleans up socket/handle and eventually invokes
  [emitClose:1167–1173](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1167-L1173):
  `if (error) self.emit('error', error); self.emit('close');`. Thus reason identity and
  truthiness control delivery; unhandled events may interrupt that later close emission.

### Warning and inspection

- events.js:585–595 compares max-listeners (default when `_maxListeners` is undefined),
  array length and `warned`. Getters/coercions can throw. Zero disables warnings and an
  already-warned array is not warned again. It sets `warned` *before* formatting
  `String(type)` and `inspect(target, { depth: -1 })`.
  [ToString](https://tc39.es/ecma262/#sec-tostring) says “If _arg_ is a Symbol, throw a TypeError
  exception”; String's direct-symbol special case does not apply to an object returning
  a symbol. Such an object works as the property key, then fails only in the warning.
- [inspect.js:857–913](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L857-L913)
  unwraps proxies without their traps, but reads/invokes custom inspection hooks with no
  catch at this point. [HTTP/2 inspect:1483–1495](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http2/core.js#L1483-L1495)
  says `if (typeof depth === 'number' && depth < 0) return this;`. Therefore ordinary
  warning formatting does not access state/localSettings/remoteSettings or their native
  implementations. User custom inspect can throw; the repro restores its own hook in finally.
- **Intrinsic inspection failures at both sites:** errors.js:1028 passes `{ depth: -1 }`
  when an invalid listener's constructor diagnostic falls back to inspect; events.js:592
  passes the same options while formatting a max-listener warning. Neither site catches
  inspection exceptions. The following paths are not adequately described as unknown
  propagation alone, and were missing from the initial 17-test audit:
  - At [inspect.js:885–932](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L885-L932),
    `return formatValue(ctx, ret, recurseTimes)` handles a non-string custom result **before**
    the circular check/registration and depth cutoff. Two objects whose hooks return each
    other cause unbounded recursion inside Node even though both callbacks return normally.
    The matching [V8 isolate.cc:1897–1913](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1897-L1913)
    uses `range_error_function()`, `MessageTemplate::kStackOverflow`, and `Throw(*exception)`.
    This recursion does not enter formatRaw's formatter try/catch: its call is after the
    custom-return branch at line 932. Both real-session sites throw exact RangeError.
  - A directly returned namespace normally reaches the guard
    `if (recurseTimes > ctx.depth && ctx.depth !== null)` at
    [inspect.js:1121–1160](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1121-L1160)
    with recurseTimes 0 and depth -1, preventing export formatting. This does **not**
    exclude other return shapes. [1053–1056](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1053-L1056)
    calls getFunctionBase before the guard; [1324–1327](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1324-L1327)
    formats a non-string function name with `formatValue(ctx, value.name)`, omitting
    recurseTimes. Thus undefined > -1 is false and the later increment produces NaN,
    bypassing the cutoff. A custom hook can normally return a function whose configurable
    name is a real namespace; the namespace export's inspect hook is then invoked.
  - [formatNamespaceObject:2003–2021](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2021)
    catches exported-property formatting errors and asserts
    `isNativeError(err) && err.name === 'ReferenceError'`. A hook throwing 17 fails that
    assertion. [internal/assert.js:8–12](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12)
    executes `throw new ERR_INTERNAL_ASSERTION(message)`;
    [errors.js:1377–1383](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383)
    registers this code with **Error**. The outer
    [formatRaw catch:1182–1185](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1182-L1185)
    says `if (!isStackOverflowError(err)) throw err;`, so it does not swallow that Error.
    Both real-session repro sites assert exact constructor/name/code and nonidentity with
    17. Hooks neither call inspect themselves nor change inspect options or built-ins.
  - Preliminary constructor-descriptor/key operations still precede the depth cutoff:
    [getConstructorName:657–693](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L657-L693)
    reads `ObjectGetOwnPropertyDescriptor(obj, 'constructor')`; namespace TDZ bindings
    can raise the intrinsic ReferenceError already included in the candidate.
    [getKeys:799–825](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L799-L825)
    catches namespace ObjectKeys failures and asserts a native ReferenceError and namespace
    before using ObjectGetOwnPropertyNames. Genuine TDZ export enumeration satisfies this
    assertion, unlike the deeper exported-object formatting failure above.
  - The separate unhandled-error branch (events.js:490–495) catches its default-depth
    inspect failures before falling back to coercion. That catch does not protect either
    of these two depth -1 diagnostic/warning sites. In the warning case insertion precedes
    the failure (count two); diagnostic failure precedes insertion (count zero).
- [errors.js:983–988](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L983-L988)
  constructs `new Error(message)` and assigns warning metadata.
  [warning.js:138–180](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/process/warning.js#L138-L180)
  accepts that Error and schedules `process.nextTick(doEmitWarning, warning)`;
  [84–86](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/process/warning.js#L84-L86)
  emits it. Its name is not DeprecationWarning, so throw-deprecation branches do not apply.

## Mechanism review and current repros

The receiver-proxy effect is now keyed to `this._events`, separately from the
listener `.listener` getter and event-key conversion. The actual HTTP/2 wrapper
and session state remain in the fixture; EventEmitter implementation reuse does
not erase wrapper guards, callback delivery or session errors. Current repros
assert intrinsic types/identity rather than redundant payloads. Current counts
and results are in the run checkpoint.

## Original audit validation (historical)

Required project: **node-24** only, by the Node inventory file inclusion. Chromium has no
node:http2 implementation. Tests use Node's actual HTTP/2 server/client on loopback and
process.execPath children, not mocks/polyfills. Children isolate capture settings/hooks;
finally blocks destroy sessions/peers and await close/server shutdown. The ReferenceError
fixture uses a temporary native ESM file, removed in finally. Each child asserts Node
24.19.0/function identity and must exit with `verified` within ten seconds.

Tests in source order:

1. `throws TypeError`: invalid listeners, exact constructor and ERR_INVALID_ARG_TYPE.
2. `throws TypeError`: invalid/frozen/revoked receivers and missing native-field receiver.
3. `throws TypeError`: invalid key conversions and symbol-producing object's warning path.
4. `throws ReferenceError`: native module-namespace uninitialized constructor binding.
5. `throws RangeError`: full sparse native Array in a forged generic receiver.
6. `throws Error`: synchronous then getter → ERR_UNHANDLED_ERROR, exact constructor/code/context.
7. `throws Error`: supplied newListener Error identity and exact constructor.
8. `throws SyntaxError`: supplied newListener SyntaxError identity and exact constructor.
9. `throws non-Error value`: supplied symbol callback failure, identity.
10. `throws non-Error value`: key/diagnostic/receiver/listener getters, identity.
11. `throws non-Error value`: custom warning inspection, identity and post-failure insertion.
12. `delivers a warning, not a thrown Error`: successful return; actual warning constructor,
    name, code absence, emitter/type/count.
13. `delivers captured newListener rejection by destroying the session`: asynchronous
    supplied Error identity and destroyed state after successful registration.
14. `delivers a numeric captured rejection as ERR_HTTP2_SESSION_ERROR`: successful return;
    awaited error-event delivery with exact Error constructor/code; destroyed state.
15. `delivers a non-Error captured rejection by identity`: successful return, awaited identity.
16. `maps native listener hooks before connect, when connected, and after destroy`: all five
    special hooks exercised across all three native-field storage states.
17. `maps inherited overloads without invoking the newly registered callback`: all event
    overload names/symbol, first/second/subsequent additions, later emit distinguished,
    reentrant newListener copy-on-write.
18. `throws RangeError`: mutually returning inspect hooks, both invalid-listener diagnostic
    and session warning; exact RangeError and pre-/post-insertion listener counts.
19. `throws Error`: returned function with namespace name bypasses depth -1; both diagnostic
    and warning produce exact Error/name/ERR_INTERNAL_ASSERTION, not the supplied 17.
    These two new cases run in isolated children with unchanged stack limits and restore
    each session's custom-inspection override in finally before native session cleanup.

Commands from repository root with `PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`:

```sh
mise run //pkgs/lib:test/node tests/node/24/http2.test.ts \
  -t '^node:http2 > ClientHttp2Session > addListener > (throws |delivers |maps )' \
  --maxWorkers 1 --reporter verbose
mise exec -- node - <<'JS'
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { createHash } = require('node:crypto');
console.log(JSON.stringify(process.versions));
for (const id of ['http2', 'internal/http2/core', 'events', 'internal/errors',
  'internal/validators', 'internal/util/inspect', 'internal/process/warning']) {
  const source = execFileSync('git', ['-C', `${process.env.HOME}/code/clones/node`, 'show',
    `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/${id}.js`], { encoding: 'utf8' });
  assert.equal(process.binding('natives')[id], source, id);
  console.log(id, createHash('sha256').update(source).digest('hex'), 'matches');
}
JS
git diff --check -- pkgs/lib/tests/node/24/http2.test.ts \
  'pkgs/lib/docs/evidence/node/24/http2/node%3Ahttp2#clienthttp2session#addlistener.md'
```

Initial validation (before adding the two extra captured-rejection reasons): **15 passed,
105 skipped, 120 total**, one file, Vitest **5.0.5**, every selected case explicitly
`node-24`. Subsequent verbose rerun before review correction: **17 passed, 105 skipped,
122 total**. Final review-correction rerun using the identical file/full-callable filter:
**19 passed, 105 skipped, 124 total**, one file passed, all 19 selected repros explicitly
marked **node-24**. Whitespace validation passed.
The MaxListenersExceededWarning printed by the warning test is expected.
Actual runtime: **Node 24.19.0**, **V8 13.6.233.17-node.51**, **nghttp2 1.69.0**, Linux.
Bare shell Node was 22.23.1, so only mise's Node and its process.execPath children were used.
All seven runtime/source comparisons passed; core SHA-256
`31ddb447c69a906be3f49d3aef7d35ff29ed7dddd466de38c98c34d531423318`, events SHA-256
`00b82a669fd6299ea5f3916473eb922a74018f96612b290024fffe399580f2c1`.
The review correction additionally reran the source-comparison body with
`['internal/assert', 'internal/util/inspect', 'internal/errors']` under `mise exec -- node -`:
all three matched, bringing the distinct verified modules to eight. Internal/assert SHA-256
`03940c9b646255b631d3a95cf2a9ad156d587db8b0ac223d1a87f3fd5531a807`.
That command again reported Node 24.19.0 / V8 13.6.233.17-node.51.

## Open questions

No behavioral blocker or shared configuration change requested. Node declaration annotation
delivery remains the coordinator's separate deferred integration task; passing these repros
does not mean the callable is annotated.
