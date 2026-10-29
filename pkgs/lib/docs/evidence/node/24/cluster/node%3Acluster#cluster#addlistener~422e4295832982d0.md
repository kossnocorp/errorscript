# node:cluster > Cluster > addListener

- ID: `node/24/cluster.test.ts::["node:cluster","Cluster","addListener"]`
- Test: `pkgs/lib/tests/node/24/cluster.test.ts`, only the selected managed suite.
- Declaration: `@types/node@24.19.2/cluster.d.ts`, interface `Cluster extends EventEmitter`
  (line 390), all eight overloads at lines 513–523: general string event; `disconnect`,
  `exit`, `fork`, `listening`, `message`, `online`, `setup`. All return `this`.
  `const cluster: Cluster`/default export is at 572–573; `node:cluster` re-exports
  `cluster` at 575–578. This is not the `Worker` suite or the separate `cluster` alias suite.
  [Published declaration](https://unpkg.com/@types/node@24.19.2/cluster.d.ts), SHA-256
  `51aecd2df90a3cffea1eb4696b33b2d78594ea2aa2138e6b9471ec4841c6c2ee`.
- Runtime mapping: the cluster singleton inherits `EventEmitter.prototype.addListener`;
  `Cluster` is a declaration interface, not a runtime constructor or callback implementation.
- Status: complete (behavior/evidence only; Node annotation delivery is deferred).
- Contract candidate: `Returns<this, unknown>` without effect parameters.
  Refined synchronous **error component**:
  `TypeError | ReferenceError | RangeError | Error | EffectsOfRegistrationHooksAndObjectOperations`.
  `Throws<F>` takes a callable, not this error union.
  The effect variable includes arbitrary JS values, not merely subclasses of `Error`.
  The newly registered callback's later invocation is not a synchronous registration effect.

## Conclusion

Synchronous intrinsic errors:

- `TypeError`: non-function listener (`ERR_INVALID_ARG_TYPE` when diagnostic formatting
  succeeds); invalid/nullish/primitive/immutable receiver; invalid property-key conversion;
  proxy invariants/revocation, failed strict writes and other malformed-receiver operations.
  The listener diagnostic itself can fail before producing its coded TypeError.
- `ReferenceError`: an invalid listener can be a real ESM namespace whose exported
  `constructor` binding is still uninitialized. The diagnostic reads that binding
  inside `addListener`; this is a native TDZ error, not a user-thrown value or a
  failure evaluating the argument/import.
- `RangeError`: this unbranded/generic method can append to a forged receiver's sparse
  listener array already of length `2**32 - 1`. Native array length assignment throws.
  This is not a claim that a normally populated cluster reaches that limit in practice.
  Also, two custom-inspection hooks that return each other's objects cause intrinsic
  diagnostic/warning inspection recursion to overflow, even with depth -1.
- `Error` (`ERR_UNHANDLED_ERROR`): with capture-rejections enabled at singleton creation,
  a `newListener` handler's returned thenable can throw a non-Error synchronously from its
  `then` getter/call. `addCatch` emits `error`; without an error handler, `emit` wraps that
  value in this Error. This is distinct from an Error supplied by a callback.
- `Error` (`ERR_INTERNAL_ASSERTION`): diagnostic or warning inspection can implicitly
  bypass depth -1 through a custom-returned function whose non-string `name` is a real
  namespace. Namespace export formatting converts a supplied hook's primitive exception
  into Node's own internal assertion Error. This is not simple identity propagation.

Arbitrary synchronous propagation includes `newListener` callbacks, their returned
thenable hooks when capture is enabled, listener `.listener` access, receiver/map
getters/setters/proxies, event-name coercion, invalid-listener diagnostic hooks, max-listener
coercion and warning inspection hooks. Error and SyntaxError callback repros assert
identity and do not attribute creation to `addListener`.

No promise or iterator is returned. Captured rejection of an existing `newListener`
handler is delivered on a later tick to the rejection-symbol handler or `error` event;
an unhandled `error` can throw on that tick, not reject `addListener`. With capture disabled,
the returned promise is not observed here; rejection remains the supplied callback's effect.
Exceeding the listener limit ordinarily schedules an **Error-valued warning**, named
`MaxListenersExceededWarning`, rather than throwing it. A warning listener can throw at
delivery time. The callback being registered runs only on a subsequent emission; those
subsequent callback throws/events/rejections are not registration throws.

Scope: Node 24.19.0, standard built-ins and cluster singleton initialization, including
the public `EventEmitter.captureRejections` setting. No domain monkey-patching, replacement
built-ins, allocation exhaustion, pre-exhausted caller stack or process termination is included.
Input-induced recursion inside the diagnostic algorithm is included. Invalid arguments and
receivers are considered; arbitrary hostile object operations retain an unknown effect.
Mutations can precede a failure (notably warning formatting), so registration is not atomic.

## Proof

### Runtime identity and revision

Node source clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, which is **not**
the tested version. All Node citations below use the explicitly selected tag `v24.19.0`,
revision `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae` (read using `git show`, without checkout).
[src/node_version.h](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L32)
defines major 24, minor 19, patch 0. The loaded runtime sources of all eight modules used
in this proof were compared byte-for-byte to this revision (see validation).

- [lib/cluster.js:28–29](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/cluster.js#L28-L29):
  `ObjectHasOwn(process.env, 'NODE_UNIQUE_ID') ? 'child' : 'primary'` chooses the implementation.
- Both [primary.js:21–32](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/cluster/primary.js#L21-L32)
  and [child.js:15–28](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/cluster/child.js#L15-L28)
  say `const cluster = new EventEmitter();` and `module.exports = cluster;`.
  Neither supplies a cluster-specific `addListener`; the platform scheduling-policy branch
  does not change inheritance. Every repro asserts the inherited function's identity.
- [events.js:607–611](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L607-L611):
  `return _addListener(this, type, listener, false);`; `on` is the same function.
  Consequently prepend-only code is not reachable from this callable.

### Validation, receiver operations, registration and coercion

- [events.js:265–267,540–599](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L540-L599)
  starts with `checkListener(listener)`, before reading `target._events`.
  [validators.js:471–474](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L471-L474):
  `if (typeof value !== 'function') throw new ERR_INVALID_ARG_TYPE(name, 'Function', value);`.
  [errors.js:1389–1459](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1389-L1459)
  registers this code with `TypeError`; constant diagnostic arguments satisfy its assertions.
  The message uses `determineSpecificType(actual)`.
  [errors.js:995–1042](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1042)
  reads `value.constructor`, its `name`, or calls inspect; these can throw arbitrary values.
  [errors.js:464–483](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L464-L483)
  explains why the coded NodeError exposes `constructor === TypeError`: its constructor
  getter returns `Base`. Tests also check the stable code, not merely base-class matching.
- A real namespace listener has `typeof === 'object'`, so it enters that diagnostic.
  `determineSpecificType` at errors.js:1025 reads `value.constructor` without a catch.
  ECMA-262 [Module namespace [[Get]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-get-p-receiver)
  delegates to [Module GetBindingValue](https://tc39.es/ecma262/#sec-module-environment-records-getbindingvalue-n-s):
  “If the binding ... is an uninitialized binding, throw a *ReferenceError* exception.”
  Matching [V8 module.cc:405–425](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L425)
  tests the export cell for a hole and executes
  `NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name)`.
  The native self-importing fixture passes its namespace to `cluster.addListener`
  before `export let constructor` initializes. Its assertion catches the ReferenceError
  around that call; fixture import then completes successfully. This establishes the
  concrete class independently of the unknown-effect component.
- `_addListener` has no receiver brand check. It reads/writes `_events`, `_eventsCount`
  and event-keyed properties. Undefined `_events` is initialized; otherwise an existing
  `newListener` is synchronously emitted **before** insertion and `_events` is read again
  afterwards. `listener.listener ?? listener` may invoke a getter. An abrupt completion
  is not caught by `_addListener`.
- Absent event: assign listener and increment count. Existing function: create two-element
  array. Existing array: `ensureMutableListenerArray`, then `existing.push(listener)`.
  [events.js:874–903](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L874-L903)
  clones arrays during active emission (`handler[kEmitting] > 0`), preserves `warned`,
  assigns the copy and otherwise returns the existing array. Reads, cloning and writes
  on hostile receiver state have ordinary JS effects. There is no rollback or catch.
- For the sparse-array repro, `existing.push(listener)` invokes native Array push, not
  a replacement. ECMA-262 [Array.prototype.push](https://tc39.es/ecma262/#sec-array.prototype.push)
  performs `? Set` for the new length, and
  [ArraySetLength](https://tc39.es/ecma262/#sec-arraysetlength) says
  “If SameValueZero(_newLength_, _numberLength_) is *false*, throw a *RangeError* exception.”
  `2**32` differs from its uint32 conversion, proving the intrinsic RangeError.
- [events.js:410–414,585–595](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L585-L595)
  reads `_maxListeners` or the default, compares it with zero and array length, then
  checks/sets `warned`. Arbitrary getter/coercion/write effects propagate. Zero disables
  the warning. Already-warned arrays do not warn again.

ECMA-262 clone revision `40ce6f206b5669e69bf010a38267ae24a2843404`
([pinned spec](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html))
supplies the relevant ordinary JS algorithms (these stable rules are corroborated by the
matching Node/V8 runtime repros):

- [GetValue](https://tc39.es/ecma262/#sec-getvalue), [ToObject](https://tc39.es/ecma262/#sec-toobject)
  and [PutValue](https://tc39.es/ecma262/#sec-putvalue): nullish base conversion throws;
  “If _succeeded_ is *false* and _refRecord_.[[Strict]] is *true*, throw a *TypeError* exception.”
  Node's events module is strict, hence primitive/immutable receiver writes fail.
- [ToPropertyKey](https://tc39.es/ecma262/#sec-topropertykey):
  “Let _key_ be ? ToPrimitive(_arg_, ~string~).”
  [ToPrimitive](https://tc39.es/ecma262/#sec-toprimitive) calls the exotic conversion with
  `? Call` and throws TypeError for an object result; [OrdinaryToPrimitive](https://tc39.es/ecma262/#sec-ordinarytoprimitive)
  tries `toString`/`valueOf`, then throws TypeError. This accounts for both invalid conversions
  and arbitrary coercion exceptions; numeric/nullish event names are not independently validated.
- [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget) and
  [Proxy get](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver)/
  [Proxy set](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-set-p-v-receiver)
  propagate getter/trap completions and enforce revoked-proxy/property invariants with TypeError.
- [String constructor](https://tc39.es/ecma262/#sec-string-constructor-string-value) special-cases
  a direct symbol, but its object argument goes through [ToString](https://tc39.es/ecma262/#sec-tostring).
  An object converting to a symbol therefore works as an event key but fails in warning
  `String(type)`. This is an error originating inside the callable, after insertion.

### newListener callbacks and capture-rejections paths

- [events.js:456–537](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L456-L537)
  obtains the event handler, invokes a single handler via `handler.apply(this, args)` or
  multiple handlers via `ReflectApply(handler[i], this, args)`. There is no catch for their
  thrown values; the array path only decrements `kEmitting` in `finally`. The event here is
  `newListener`, not `error`, so ordinary registration does not itself take emit's unhandled
  error branch. Callback mutation/reentrancy is followed by rereading the event map.
- [events.js:248–254,332–353](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L332-L353)
  sets capture false by default and copies the prototype capture setting on initialization.
  Tests enable it using the public API **before requiring cluster** in isolated processes.
- Non-null handler results reach
  [addCatch:356–375](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L356-L375).
  It returns immediately when capture is false; otherwise it reads `promise.then` and
  invokes a callable then with a rejection callback. Its catch says `that.emit('error', err)`.
  Thus a synchronous then getter/call failure can invoke error/errorMonitor callbacks,
  rethrow an Error by identity, or produce a new `ERR_UNHANDLED_ERROR` for a non-Error.
  The latter branch is explicit at events.js:467–500: `throw er` for an Error, otherwise
  `const err = new ERR_UNHANDLED_ERROR(stringifiedEr); err.context = er; throw err;`.
  Its inspection attempt catches inspection failures, but subsequent message coercion
  can itself propagate a hostile value's error.
  [errors.js:1856–1863](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1856-L1863)
  registers `ERR_UNHANDLED_ERROR` with `Error`.
- The rejection callback schedules `process.nextTick(emitUnhandledRejectionOrErr, ...)`.
  [events.js:378–397](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L378-L397)
  invokes `ee[kRejection](err, type, ...args)` if present, else emits `error` with capture
  temporarily disabled and restored in `finally`. The source explicitly notes a throwing
  error handler “will end up in 'uncaughtException'”. This is callback/event delivery,
  not an awaited rejection of the registration result.

### Warning path

- [events.js:587–594](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L587-L594)
  creates the warning only after insertion and `existing.warned = true`. Message creation
  evaluates `String(type)` and `inspect(target, { depth: -1 })` synchronously.
  [inspect.js:857–913](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L857-L913)
  unwraps proxies without invoking their traps, but then reads `value[customInspectSymbol]`
  and invokes the user hook via `FunctionPrototypeCall`. No catch hides its error here;
  custom results can recursively be inspected. This occurs even at depth -1.
- **Namespace custom-return/depth analysis:** both the invalid-listener fallback
  (errors.js:1028) and the warning (events.js:592) explicitly pass `{ depth: -1 }`.
  A custom hook returning a non-string namespace triggers
  `return formatValue(ctx, ret, recurseTimes)` at
  [inspect.js:904–910](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L904-L910),
  preserving the same context/depth and recursion count (initially zero).
  [inspect.js:1121–1160](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1121-L1160)
  selects `formatNamespaceObject`, but **before invoking it** checks
  `if (recurseTimes > ctx.depth && ctx.depth !== null)` and returns a short label.
  Hence simply returning a namespace does not inspect its exported objects and cannot
  reach the nested-export assertion at
  [inspect.js:2003–2021](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2021):
  `assert(isNativeError(err) && err.name === 'ReferenceError')`.
  [internal/assert.js:8–12](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12)
  says `throw new ERR_INTERNAL_ASSERTION(message)` when its condition fails;
  [errors.js:1377–1383](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383)
  registers that code with `Error`. Thus the assertion produces `ERR_INTERNAL_ASSERTION`
  (base Error) at deeper inspection,
  but is **excluded for these direct depth:-1 namespace-return paths**.
  The added repro returns a native namespace containing a throwing custom-inspection
  export from both sites: the diagnostic still throws coded TypeError, warning delivery
  succeeds, and the exported hook's invocation counter stays zero.
- Depth -1 does **not** skip all preliminary operations. `getConstructorName` and
  `getKeys` precede the depth guard. An uninitialized `constructor` namespace export
  can therefore still throw ReferenceError during constructor descriptor lookup:
  [inspect.js:657–693](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L657-L693)
  executes `ObjectGetOwnPropertyDescriptor(obj, 'constructor')`;
  [namespace [[GetOwnProperty]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-getownproperty-p)
  reads the export binding. Matching [V8 module.cc:428–445](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L428-L445)
  creates `NewReferenceError(MessageTemplate::kNotDefined, name)` for a hole-valued export.
  [inspect.js:799–825](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L799-L825)
  catches namespace `ObjectKeys` failures separately, asserts a native ReferenceError
  and a real namespace, then uses `ObjectGetOwnPropertyNames`. For genuine namespace
  exports, own-descriptor retrieval only reads the binding: a TDZ error satisfies this
  assertion, and initialized exported values do not invoke their hooks during key enumeration.
  This is not the deeper namespace-formatting internal assertion path. Hostile ordinary
  object/proxy/custom-inspection operations elsewhere still have their stated effects.
- **Implicit depth bypass through a function name:** the direct-return exclusion above
  does not generalize to other return shapes. Both registration inspection sites can
  receive a custom hook's returned function whose configurable `name` is a native namespace.
  [inspect.js:1053–1056](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1053-L1056)
  invokes `getFunctionBase` before the depth guard.
  [getFunctionBase:1324–1327](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L1324-L1327)
  executes `formatValue(ctx, value.name)` for a non-string name, **omitting recurseTimes**.
  Thus `undefined > -1` is false and the subsequent `recurseTimes += 1` yields NaN;
  the namespace formatter actually runs. Its exported object's custom hook throws 17,
  which the namespace formatter replaces with `ERR_INTERNAL_ASSERTION` as proved above.
  The new repro exercises **both** the invalid-listener diagnostic and warning paths,
  asserting exact Error constructor/name/code and nonidentity with 17. No hook calls
  `inspect` itself, no options are overridden, and no built-in is replaced. Warning-path
  failure occurs after insertion; diagnostic failure precedes insertion. The earlier
  statement suggesting no implicit depth bypass was possible was incorrect.
- **Custom-return cycle:** at inspect.js:885–932, a non-string custom result recursively
  calls `formatValue` with the same depth **before** the ordinary circular-value check
  and registration. Two normally returning hooks can return each other's objects forever.
  This bypasses the depth cutoff without any supplied hook throwing. Matching
  [V8 isolate.cc:1897–1913](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1897-L1913)
  uses `range_error_function()`, `MessageTemplate::kStackOverflow`, then `Throw(*exception)`.
  The added repro asserts exact RangeError for the diagnostic and warning sites. This
  is algorithmic recursion induced by input shape, not an already exhausted caller stack.
- In the separate capture-enabled unhandled-error path,
  events.js:490–495 calls default-depth `inspect(er)` inside a try/catch: a namespace
  formatter's internal assertion is caught there, and subsequent fallback coercion
  determines the outward error. This catch does not exist around the diagnostic/warning
  inspection sites and does not exclude their intrinsic internal assertion Error.
- [errors.js:983–988](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L983-L988):
  `const err = new Error(message); ObjectAssign(err, errorProperties); return err;`.
  The own name is `MaxListenersExceededWarning`, not `DeprecationWarning`; there is no code.
- [warning.js:138–180](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/process/warning.js#L138-L180)
  accepts that real Error, with no type/code options, and schedules
  `process.nextTick(doEmitWarning, warning)`. The deprecation/throw-deprecation branches
  do not apply to this name.
  [warning.js:84–86](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/process/warning.js#L84-L86)
  says `process.emit('warning', warning)`. Warning listeners' own exceptions occur later.

## Mechanism review and current repros

The receiver-proxy propagation case now targets `this._events`; listener `.listener`
access and event-key conversion remain separate dependencies. Existing EventEmitter
source mapping and guards still apply. Reuse that proof for aliases only after
checking each receiver's wrappers/state (HTTP/2 sessions, for example). The current
repros assert intrinsic types or supplied identity, not redundant diagnostic
payloads; current counts/results are in the run checkpoint.

## Original audit validation (historical)

All 17 cases are Node-only by file/project inclusion; Chromium has no `node:cluster`
implementation and is not required. Tests use actual `process.execPath` children,
standard built-ins, no mocks/polyfills. Each child asserts Node 24.19.0 and inherited
function identity. Every process owns its singleton, settings/listeners, and any hook
properties; process exit releases them without changing the Vitest parent's state.
Each exec has a 10-second timeout and must exit successfully with a verification marker.

Cases in source order (duplicate titles distinguish branches by their bodies):

1. `throws TypeError`: invalid listeners, exact constructor and `ERR_INVALID_ARG_TYPE`.
2. `throws TypeError`: nullish/primitive/frozen receivers, revoked proxy; exact constructor.
3. `throws ReferenceError`: native self-import namespace's uninitialized `constructor`
   binding read during listener validation; exact constructor/name and absent code.
   The temporary `.mjs` file is created beneath `node:os.tmpdir()` and removed in
   `finally`; native import succeeds.
4. `throws TypeError`: native namespace returned by diagnostic and warning inspect hooks;
   checks the depth:-1 exclusion for **direct** namespace returns only.
5. `throws Error`: function-name namespace detour at both diagnostic and warning sites;
   exact Error/name/`ERR_INTERNAL_ASSERTION`, not supplied 17; checks insertion timing.
6. `throws RangeError`: normally returning inspection hooks form a return cycle;
   exact constructor at diagnostic and warning sites, with no pre-exhausted caller stack.
7. `throws TypeError`: failed event coercion and symbol-producing object's warning coercion;
   verifies insertion preceded the warning failure.
8. `throws Error`: capture-enabled then getter throws non-Error; exact Error constructor,
   `ERR_UNHANDLED_ERROR`, original `context` identity.
9. `throws RangeError`: forged full sparse listener array; exact constructor.
10. `throws Error`: supplied single `newListener` error; identity plus exact constructor.
11. `throws SyntaxError`: supplied array-path `newListener` error; identity plus constructor.
12. `throws non-Error value`: callback throws a supplied symbol; identity.
13. `throws non-Error value`: event conversion, invalid-listener diagnostic getter, receiver
   proxy and listener `.listener` getter; supplied object identity in each case.
14. `throws non-Error value`: warning inspect hook; identity and post-insertion count.
15. `delivers a warning, not a thrown Error`: successful return; awaits actual warning,
    exact Error constructor, name, missing code, emitter/type/count.
16. `delivers captured newListener rejection asynchronously`: successful return;
    awaits error-event delivery and rejection-symbol-handler delivery, preserving identity.
17. `maps inherited registration without invoking the registered callback`: successful
    first/second/subsequent additions; later emit is explicitly distinguished and asserts identity.

Commands (repository root, `PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`):

```sh
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/cluster.test.ts \
  -t '^node:cluster > Cluster > addListener > (throws |delivers |maps )' --maxWorkers 1
mise run //pkgs/lib:test/node tests/node/24/cluster.test.ts \
  -t '^node:cluster > Cluster > addListener > (throws |delivers |maps )' --maxWorkers 1 --reporter verbose
git diff --check -- pkgs/lib/tests/node/24/cluster.test.ts \
  'pkgs/lib/docs/evidence/node/24/cluster/node%3Acluster#cluster#addlistener~422e4295832982d0.md'
```

Final correction run (13:03:11): Vitest **5.0.5**, **node-24**,
**1 file passed, 17 tests passed, 32 skipped** (the other callable TODOs), 49 total.
Earlier runs passed 12, 13, then 15 cases before the function-name depth-bypass correction.
The final verbose run explicitly marked all 17 selected cases `|node-24|`.
Whitespace validation passed. The two real MaxListenersExceededWarnings on stderr are expected.
Runtime: **Node 24.19.0**, **V8 13.6.233.17-node.51**, Linux. A bare shell `node -v`
reported 22.23.1, so it was not used for validation: `mise` and all test children used 24.19.0.

Runtime/source matching was additionally executed with `mise exec -- node -` and this body:

```js
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const ids = ['cluster', 'internal/cluster/primary', 'internal/cluster/child', 'events',
  'internal/errors', 'internal/validators', 'internal/util/inspect', 'internal/process/warning'];
for (const id of ids) {
  const source = execFileSync('git', ['-C', `${process.env.HOME}/code/clones/node`, 'show',
    `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/${id}.js`], { encoding: 'utf8' });
  const runtime = process.binding('natives')[id];
  require('node:assert/strict').equal(runtime, source, id);
  console.log(id, crypto.createHash('sha256').update(runtime).digest('hex'), 'matches');
}
```

All eight comparisons succeeded; `events` SHA-256 was
`00b82a669fd6299ea5f3916473eb922a74018f96612b290024fffe399580f2c1`.
No configuration/dependency/helper changes are requested. No declaration annotations were
installed; the coordinator must separately integrate the candidate when Node annotation
delivery is supported.
