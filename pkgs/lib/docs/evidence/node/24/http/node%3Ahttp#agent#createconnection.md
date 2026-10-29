# node:http > Agent > createConnection

- ID: `node/24/http.test.ts::["node:http","Agent","createConnection"]`
- Test: `pkgs/lib/tests/node/24/http.test.ts`, selected managed suite only.
- Declaration: installed `@types/node` **24.19.2**, `http.d.ts:1770–1773`:
  `createConnection(options: ClientRequestArgs, callback?: (err: Error | null, stream: stream.Duplex) => void): stream.Duplex | null | undefined`.
  This is the sole Agent method signature; `node:http` re-exports `http` at
  2244–2245. The similarly named optional callback property in
  `ClientRequestArgs` (208–213) is NOT another overload or the implementation
  audited here. Overrides supplied by subclasses are not this built-in.
- Runtime mapping: `Agent.prototype.createConnection`, a proxy-aware wrapper
  in `lib/_http_agent.js:295–322`, NOT an alias of `net.createConnection` in this release.
- Status: **blocked** (partial verified evidence; literal TODO retained).
- Contract candidate: conservatively
  `Returns<stream.Duplex | null | undefined, unknown>` for the declaration.
  Synchronous intrinsic lower bound: `TypeError | RangeError | ReferenceError | Error`;
  arbitrary synchronous supplied-code effects prevent a finite exhaustive union.
  `SyntaxError` below is supplied-code propagation, not an intrinsic parser error.
  No declaration annotation is delivered by this audit. Node delivery remains deferred.

## Conclusion

The direct path normalizes arguments, reads the receiver's proxy configuration,
evaluates a host/port diagnostic template even with debug logging disabled, then
constructs and connects a real socket. Bad receivers, conversions, socket/stream,
timer, endpoint, DNS, and signal options can fail synchronously. Error-message
construction itself runs inspection, so it is not safe to limit validation errors
to their intended `TypeError`/`RangeError` constructors. Native namespace diagnostics
produce `ReferenceError`; namespace inspection can replace a supplied thrown value
with a newly created `Error` (`ERR_INTERNAL_ASSERTION`). Closed `BoundSocket`
adoption produces `Error` (`ERR_SOCKET_HANDLE_ADOPTED`).

Getters, proxies, coercions, diagnostic constructor/name reads, custom inspectors,
custom lookup functions, and `onread.buffer` generators can propagate arbitrary
values synchronously. Lookup callbacks may run synchronously or later; callback
delivery time determines when their property/conversion/listener effects occur.
An error **reported to** the lookup callback is instead forwarded to the socket's
`error` event. The repro proves identity even for a non-Error object.

Network failures, blocked IPs, DNS result validation, multi-address aggregation,
abort, and TLS failures generally use deferred socket events, not a promise returned
by this callable. Verified event types here are `Error`, `AggregateError`,
`TypeError`, and `RangeError`. These events are NOT added to the initiating call's
`Returns` merely because they happen later. Without a listener, EventEmitter can
turn an `error` event into an uncaught exception; that is still a separate channel.
The optional function is registered as a `connect` listener on the TCP path and a
`secureConnect` listener on the HTTPS-proxy path. The declaration comment saying the
default implementation ignores it is not accurate for this runtime: it is not an
error-first completion callback. Listener throws occur at event delivery, not
automatically during `createConnection`.

Assumptions: ordinary unmodified built-ins, real Agent construction, no forged
private handles/proxy configuration symbols, no OOM/process-fatal conditions.
Invalid public options and receivers are included despite needing TS casts.
Agent-constructor proxy URL parsing errors are not attributed to this method.
Linux x64 is the tested platform; complete native/platform and HTTPS-proxy coverage
is outstanding, as listed below.

## Proof

### Version and mapping

Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; **all Node citations
below instead use** matching commit `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`.
[src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27)
says `NODE_MAJOR_VERSION 24`, `NODE_MINOR_VERSION 19`, `NODE_PATCH_VERSION 0`.
The tested runtime's embedded `net` source was also compared byte-for-byte with
`git show <revision>:lib/net.js` and matched.

1. [lib/_http_agent.js:295–320](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/_http_agent.js#L295-L320):
   `const shouldUseProxy = checkShouldUseProxy(this[kProxyConfig], options)`;
   ``debug(`http createConnection should use proxy for ${options.host}:${options.port}:`, shouldUseProxy)``;
   `return net.createConnection(...args)` / `return net.connect(connectOptions, cb)` /
   `return lazyTLS().connect(connectOptions, cb)`.
   Null/undefined receivers fail on property access; the template invokes conversions
   before any socket is constructed. Proxy/non-proxy selection is real behavior here.
   ECMA-262 [ToString](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5653-L5675)
   says “If _arg_ is a Symbol, throw a *TypeError* exception” and propagates
   `? ToPrimitive(_arg_, ~string~)` for objects, explaining both conversion repros.
2. [lib/internal/http.js:138–191,239–248](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/http.js#L138-L191):
   `const host = hostname.toLowerCase()` with a nonempty bypass list explains the
   numeric-host TypeError. A proxy's connection options contain only URL-derived
   host and port; arbitrary request TLS options are NOT blindly forwarded.
   `socketPath` bypasses proxy selection. Supported protocols are restricted during
   Agent construction (212–231), so the final wrapper assertion is unreachable for
   an ordinary unmodified Agent.
3. [lib/net.js:244–255,290–323](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L244-L323):
   `const socket = new Socket(options)`; `socket.setTimeout(options.timeout)`;
   `return socket.connect(normalized)`. Normalization accepts object, port, and path
   forms at runtime, though this Agent declaration only exposes the object form.
   A non-function final argument is discarded, not validated as a listener.

### Synchronous validation and diagnostics

4. [lib/net.js:485–651](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L485-L651):
   object/readableObject/writableObject modes throw `ERR_INVALID_ARG_VALUE`;
   keepalive delay uses `validateNumber`; `options = { ...options }` invokes own-key,
   descriptor, and value traps; `stream.Duplex.call(this, options)` validates stream
   options. `validateInt32(options.typeOfService, ..., 0, 255)` and block-list checks
   add range/type branches. `createHandle(fd)` at 188–203 validates a nonnegative
   int32 and rejects non-PIPE/non-TCP descriptors with `ERR_INVALID_FD_TYPE`.
   `throw new ErrnoException(err, 'open')` (569) and Windows `setBlocking` (578)
   establish further host-dependent Error paths, not fully validated here.
5. [lib/net.js:475–480,545–548](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L475-L548):
   adoption calls the BoundSocket consume operation; a closed/already-consumed
   handle executes `throw new ERR_SOCKET_HANDLE_ADOPTED()`. The repro closes a
   successfully bound loopback fixture before invoking the audited method.
6. [lib/internal/streams/duplex.js:85–134](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/duplex.js#L85-L134)
   constructs readable/writable states, installs supplied stream callbacks and
   calls `addAbortSignal`. [state.js:32–45](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/state.js#L32-L45)
   throws `ERR_INVALID_ARG_VALUE` for invalid high-water marks.
   [readable.js:298–312](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/readable.js#L298-L312)
   has `throw new ERR_UNKNOWN_ENCODING(defaultEncoding)` and constructs a decoder
   for `encoding`; writable state validates its default encoding too.
   [add-abort-signal.js:26–57](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/add-abort-signal.js#L26-L57)
   rejects invalid signal shapes and destroys with `new AbortError(...)` on abort.
7. [stream_base_commons.js:240–267](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/stream_base_commons.js#L240-L267)
   calls `getTimerDuration`; [timers.js:451–465](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/timers.js#L451-L465)
   uses `validateNumber` and throws `ERR_OUT_OF_RANGE` for negative/nonfinite durations.
8. [lib/net.js:1427–1655](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L1427-L1655):
   `throw new ERR_MISSING_ARGS(['options', 'port', 'path'])`, path/host string
   validation, local-address IP validation, numeric localPort, port validation,
   boolean auto-selection and positive int32 attempt timeout. A numeric host is
   scheduled with `process.nextTick`; hostnames invoke a supplied or default lookup
   immediately. [lib/dns.js:143–200](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/dns.js#L143-L200)
   validates hints/family, so family 7 fails before any DNS request.
9. [validators.js:118–131,188–200,436–445](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L118-L131)
   distinguishes `ERR_INVALID_ARG_TYPE`, `ERR_OUT_OF_RANGE`, and `ERR_SOCKET_BAD_PORT`.
   [errors.js:1389–1494](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1389-L1494),
   [1588–1660](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1588-L1660),
   [1746–1767](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1746-L1767),
   and [1866](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1866)
   register the exact base constructors asserted by the tests.
10. [errors.js:995–1028,1456–1467](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1028):
    `value.constructor`, `value.constructor.name`, and
    `lazyInternalUtilInspect().inspect(value)` can run supplied code while formatting
    validation errors. [inspect.js:885–910](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L885-L910)
    calls a custom inspector without suppressing its exception. Identity repros
    cover a supplied SyntaxError and arbitrary non-Error values (including constructor
    getter failures); they do not classify those values as intrinsic.
11. [inspect.js:657–674](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L657-L674)
    reads `ObjectGetOwnPropertyDescriptor(obj, 'constructor')`. On the fixture's
    native namespace this reads an uninitialized lexical export while the callable
    is constructing its objectMode validation error. ECMA-262 clone HEAD
    `40ce6f206b5669e69bf010a38267ae24a2843404`,
    [namespace [[GetOwnProperty]] / [[Get]]](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L15645-L15736)
    forwards to the binding read; [Module GetBindingValue](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L11712-L11735)
    says “If the binding ... is an uninitialized binding, throw a *ReferenceError*
    exception.” Thus this is not an argument-evaluation failure. No getter in the
    fixture explicitly throws ReferenceError.
12. [inspect.js:2003–2010](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2010)
    catches errors formatting namespace exports and asserts
    `isNativeError(err) && err.name === 'ReferenceError'`. An exported object's
    inspector throwing 42 fails that assertion.
    [internal/assert.js:7–11](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L7-L11)
    creates `ERR_INTERNAL_ASSERTION`; [errors.js:1377–1383](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383)
    registers it as **Error**, not AssertionError. The repro verifies exact Error
    constructor/code, not merely `instanceof Error`.

### Propagation and delivery channels

13. [net.js:326–347](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L326-L347)
    invokes `userBuf = bufGen()` during handle initialization without a catch.
    This accounts for synchronous `onread.buffer` propagation, also reproed by identity.
14. [net.js:1624–1775](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L1624-L1775)
    invokes custom lookup, reads callback-result records, and for reported errors
    executes `process.nextTick(connectErrorNT, self, err)`. Invalid IPs create
    TypeError and invalid address families create RangeError, also scheduled to
    destroy. [destroy.js:115–129,152–170](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/destroy.js#L115-L170)
    ultimately executes `self.emit('error', err)`; no returned promise is involved.
15. [net.js:1251–1425](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L1251-L1425)
    creates bind/connect errors and destroys the socket; a blocked address executes
    `self.destroy(new ERR_IP_BLOCKED(address))`. The multi-address path accumulates
    errors then `self.destroy(new NodeAggregateError(context.errors))`.
    [errors.js:198–210](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L198-L210)
    explicitly exposes `AggregateError` as its constructor; tests check that and each
    constituent Error/code. Both blocklist fixtures use loopback only and initiate
    no actual connection.
16. [net.js:1441–1448](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L1441-L1448)
    publishes `net.client.socket` then calls `this.once('connect', cb)`.
    [diagnostics_channel.js:160–170](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/diagnostics_channel.js#L160-L170)
    catches subscriber throws and schedules `triggerUncaughtException`, rather than
    propagating them synchronously. Test observers only record real sockets for
    cleanup, never mutate behavior or throw. Stream construct callbacks are likewise
    scheduled and caught into destruction in
    [destroy.js:245–305](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/streams/destroy.js#L245-L305).
    [events.js:467–533](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L467-L533)
    says `throw er; // Unhandled 'error' event`, wraps unhandled non-Errors in
    `ERR_UNHANDLED_ERROR`, and otherwise invokes listeners with `handler.apply(this, args)`
    / `ReflectApply` without catching their thrown values. This explains the separate
    event-listener exception channel; repros always attach an error listener.

## Mechanism review and current repros

This remains a partial audit. The proxy fixture now targets `options.objectMode`
instead of failing at an unidentified first read. It is separate from `host` get,
options own-key enumeration, host coercion, lookup invocation, invalid-delay
diagnostics and `onread.buffer` invocation. Callback/error-event delivery is still
recorded separately from synchronous throws. Current counts/results are in the
run checkpoint; the original payload assertions below are historical.

## Original audit validation (historical)

Command from repository root (only this file and escaped full callable boundary):

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise run //pkgs/lib:test/node tests/node/24/http.test.ts \
  -t '^node:http > Agent > createConnection > (?:.* > )?throws ' --maxWorkers 1
```

Final result: **node-24: 31 passed, 69 skipped, 1 file passed** (100 discovered tests).
The skipped tests are the retained TODO and other callable scaffolds; no skipped
test is counted as proof. All 31 selected cases have `node-only` tags. Chromium is
not applicable to this Node library inventory.

- 19 validation scenarios: missing endpoint, object mode, high-water mark, encoding,
  descriptor range, keepalive delay, type of service, timeout, port range/type, path
  type, host NUL, local address, auto family, family timeout, lookup type, DNS family,
  block list, abort signal. Each asserts the precise constructor and stable Node code.
- Receiver/template conversion and proxy-bypass hostname: synchronous TypeError.
- Native uninitialized namespace: synchronous ReferenceError, asserted inside a real
  Node subprocess with `process.execPath`. A temporary self-importing ESM file is
  removed in finally. Initial Vitest-transformed import produced TypeError instead;
  a dynamic Function import lacked an import callback; a native run then passed the
  constructor assertion but disproved the assumed message wording. Final test uses
  exact constructor and stable name, not a guessed message. All three exploratory
  full filtered runs had 27 passes / 1 failure; those failures were fixture/assertion
  issues and are not callable error evidence.
- Namespace diagnostic assertion and closed bound-socket adoption: separate
  synchronous Error repros, exact `ERR_INTERNAL_ASSERTION` / `ERR_SOCKET_HANDLE_ADOPTED`.
- Custom inspector: supplied SyntaxError identity. Seven non-Error identity scenarios:
  host getter, proxy get, proxy ownKeys, coercion, lookup, diagnostic constructor getter,
  and onread buffer generation.
- Five deferred-event cases: blocked address Error, multiple blocked addresses
  AggregateError (two constituent Errors), supplied lookup error identity, invalid
  lookup address TypeError, invalid lookup family RangeError. Promises bridge event
  observation and resolve with the delivered value; these are not `.rejects` tests
  because the callable does not return a rejected promise.

An intermediate corrected run had 30 passes / 69 skipped; the final added
BoundSocket repro brings the selected count to 31. Sockets observed by diagnostics
are destroyed in finally even after synchronous failure; subscriptions are removed,
Agents destroyed, BoundSockets disposed, and no external network is used.

Runtime: `/home/koss/.local/share/mise/installs/node/24.19.0/bin/node`, Node **v24.19.0**,
V8 **13.6.233.17-node.51**, OpenSSL **3.5.7**, libuv **1.52.1**, Linux x64;
Vitest **5.0.5**. Initial bare `node --version` was v22.23.1 and was NOT used for
validation. `mise exec -- node -p 'JSON.stringify(process.versions)'` verified the
actual toolchain. A further `mise exec -- node -e ...` compared
`process.binding('natives').net` with the pinned source: `netSourceMatches: true`.

## Open questions / precise blocker

Coordinator cleanup correction: synchronous captures restore the original
`Error.stackTraceLimit` descriptor after diagnostic failures. The final grouped
run in the checkpoint validates this state-restoration change.

This is not a complete reachable-path proof. Keep the TODO until the following are resolved:

1. HTTPS proxy dispatch reaches
   [internal/tls/wrap.js:1832–1938](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/tls/wrap.js#L1832-L1938),
   which executes `tls.createSecureContext(options)`, constructs TLSSocket, and
   installs the optional callback on `secureConnect`.
   [secure-context.js:129–180](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/tls/secure-context.js#L129-L180)
   configures default ciphers/CA through native OpenSSL calls. Follow default-context,
   CA/configuration, and TLS setup failures to matching native implementations and
   validate reachable synchronous versus event failures with a local TLS proxy.
   Do not claim arbitrary request-provided key/certificate parsing errors: the wrapper
   copies only its proxy connection options. No HTTPS-proxy failure repro is present.
2. Finish native fd-open, platform-specific Windows setBlocking, adopted-handle,
   IPC connect, address conversion, and getsockname branches. Source explicitly
   identifies hard-to-fabricate open failures; these must not be turned into a
   claim of no errors. No Windows runtime was provided; document a justified
   platform contract or request a Windows project before claiming cross-platform validation.
3. Complete/repro remaining deferred abort, TLS, stream-hook, ordinary native network
   failures, diagnostic-subscriber exception delivery, and connection-listener
   propagation as appropriate. Do not fold these into the synchronous contract.
   No exhaustive claim about all custom diagnostic/namespace/stream callback paths
   is made; conservative unknown propagation remains necessary.

No shared configuration change is needed to run the verified Linux cases. Any
additional platform runner or TLS fixture/helper must be coordinator-owned; this
worker did not edit shared runners, dependencies, helpers, manifests, or annotations.
