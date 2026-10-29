# node:net > BlockList > addAddress

- ID: `node/24/net.test.ts::["node:net","BlockList","addAddress"]`
- Test: `pkgs/lib/tests/node/24/net.test.ts`, the exact managed suite above.
- Declarations: `@types/node@24.19.2/net.d.ts` lines 884, 891–899:
  `addAddress(address: string, type?: IPVersion): void` and
  `addAddress(address: SocketAddress): void`; `IPVersion = "ipv4" | "ipv6"`.
  The `net` module declaration (line 15) is re-exported by `node:net`
  (1195–1197). [Versioned declaration](https://unpkg.com/@types/node@24.19.2/net.d.ts),
  SHA-256 `cd541a9b163154df9713f5669c157e330f6957f44915a097c4d99e9f4df616dd`.
- Runtime mapping: `lib/net.js:2722–2724` lazily exports the class from
  `internal/blocklist`; both overloads invoke `BlockList.prototype.addAddress`,
  `lib/internal/blocklist.js:73–83`, then native `SocketAddressBlockListWrap::AddAddress`.
- Status: complete (evidence/repros, not declaration annotation).
- Contract candidate: `Returns<void, unknown>`. More descriptively:
  `Returns<void, TypeError | Error | RangeError | ReferenceError | E>` where `E`
  includes arbitrary effects from supplied objects, proxies, getters, diagnostic
  custom inspection and its returned objects. Without an effect bound, `E` is
  `unknown`; the displayed concrete types are not an exhaustive finite contract.

## Conclusion

All effects here are **synchronous**. There are no promises, deferred iterators,
error-first callbacks, sockets opened, or error events initiated by this method.

- `TypeError`: Non-string address/family: `ERR_INVALID_ARG_TYPE`; unsupported
  string family: `ERR_INVALID_ARG_VALUE`; invalid receiver/property access,
  revoked/invariant-violating proxy, malformed diagnostic `constructor`: uncoded.
- `Error`: Native IPv4/IPv6 parsing failure (including address/family mismatch):
  `ERR_INVALID_ADDRESS`.
- `ReferenceError`: Diagnostic access to an uninitialized `constructor` export
  of a real ESM namespace; uncoded.
- `RangeError`: Diagnostic custom-inspection return cycle exhausts the JS stack;
  uncoded.
- `Error`: Inspection `ObjectKeys` failure other than the supported ESM
  ReferenceError is replaced by `ERR_INTERNAL_ASSERTION`.

Identity-preserving propagation can occur before validation, while reading the
receiver handle or the address handle, or during validation error formatting.
In particular, passing an object is not guaranteed to produce the advertised
validation TypeError: constructing its message can itself fail. The internal
assertion case deliberately **does not** preserve the supplied exception.
No intrinsic `SyntaxError` was established; invalid IP syntax produces the
native `Error` above, not a parser `SyntaxError`. Supplied code may throw one
(or any other value), which remains covered by `E`, not a fabricated intrinsic.

Scope: Node 24.19.0, normal built-in prototypes and default inspection options.
No global monkey-patching, forged/corrupted native handles, OOM, process
termination, or external asynchronous interruption is included in the error
contract. Ordinary invalid receivers and hostile user inputs are included.
The native handle `CHECK` failure on fabricated address handles is a process
termination, not an additional JavaScript throw. No claim is made that a finite
set describes arbitrary custom-inspection implementations.

## Proof

### Revisions

All Node links below use **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**.
The local Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`,
so research used `git show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:<path>`,
not HEAD files. [`src/node_version.h:25–27`][version] specifies 24.19.0;
[`deps/v8/include/v8-version.h:11–14`][v8version] specifies 13.6.233.17,
matching the tested V8 `13.6.233.17-node.51`. libuv runtime is 1.52.1.
ECMA-262 local revision: `40ce6f206b5669e69bf010a38267ae24a2843404`.

### Dispatch, validation, and native insertion

1. [`lib/net.js:2722–2728`][net] returns the actual internal classes, not a
   callback/interface implementation. [`blocklist.js:73–83`][blocklist] says
   `if (!SocketAddress.isSocketAddress(address))`, validates both arguments,
   constructs `new SocketAddress({ address, family })`, and finally calls
   `this[kHandle].addAddress(address[kSocketAddressHandle]);`. The JS method
   returns `undefined` regardless of the native boolean result. A real
   SocketAddress bypasses **all** family validation; default family is `ipv4`.
2. [`socketaddress.js:45–85`][socketaddress] implements the apparent brand check
   as `return value?.[kHandle] !== undefined;`. Nullish inputs pass through to
   string validation; non-null proxies/getters can throw during the symbol read.
   No string coercion is attempted on the original address/family.
   [`validators.js:164–168`][validators] states
   `throw new ERR_INVALID_ARG_TYPE(name, 'string', value);` for a non-string.
   [`errors.js:1389–1467`][errors] defines that code as `TypeError` and the
   default `ERR_INVALID_ARG_VALUE` constructor as `TypeError` (its separate
   `.RangeError` alternative is **not** invoked here).
3. SocketAddress receives a fresh ordinary options object, validated by
   `validateObject` (`validators.js:252–282`). Its supplied address/family are
   already primitive strings. `family.toLowerCase()` precedes a switch with
   only `ipv4` and `ipv6` cases; the default throws
   `new ERR_INVALID_ARG_VALUE('options.family', options.family)`.
   The other constructor checks are not uncontrolled inputs: port and flowlabel
   default to zero. `validatePort` (436–445) permits zero; `validateUint32`
   (142–155) permits zero with `positive = false`. `markTransferMode`
   ([`js_transferable.js:91–98`][transfer]) merely sets a private-symbol property
   on the fresh extensible instance. Constructor `.detail()` writes four own
   pre-existing properties of another fresh object (`node_sockaddr.cc:810–838`),
   not user setters. Address formatting uses `uv_inet_ntop` on the validated
   native address ([`node_sockaddr-inl.h:40–49`][sockaddr-inl]).
4. [`node_sockaddr.cc:787–807`][sockaddr] constructs the address and says
   `if (!SocketAddress::New(family, *address, port, addr.get()))`
   `return THROW_ERR_INVALID_ADDRESS(env);`.
   [`node_errors.h:93,222`][native-errors] registers
   `V(ERR_INVALID_ADDRESS, Error)` / `"Invalid socket address"`.
   `SocketAddress::New` / `ToSockAddr` (43–78) select `uv_ip4_addr` or
   `uv_ip6_addr` and test their result against zero.
   [`uv-common.c:254–297`][uv-common] calls `uv_inet_pton`; IPv6 zone names use
   `atoi` on Windows or `if_nametoindex` elsewhere, and explicitly say
   `unknown interface (id=0) is silently ignored`. These branches do not add
   JavaScript exceptions. [`inet.c:146–297`][inet] returns `UV_EINVAL` for
   invalid digit counts, octets, compression, lengths, or family-specific
   syntax. Both families are exercised; there is no DNS/network request.
5. [`node_sockaddr.cc:592–605,696–709`][sockaddr] binds `addAddress` and calls
   `wrap->blocklist_->AddSocketAddress(addr->address());`, then returns true.
   The address is a native object guarded by `CHECK(SocketAddressBase::HasInstance(...))`.
   `ASSIGN_OR_RETURN_UNWRAP` ([`base_object.h:249–254`][unwrap]) returns on a
   missing native pointer; it does not construct another error. Insertion
   (`node_sockaddr.cc:397–404`) locks, creates a rule, inserts it, and updates
   the address map. It does not reject duplicate addresses or invoke JS.
   Thus no native RangeError/range-order error from `addRange` or prefix error
   from `addSubnet` belongs to this callable.

### JavaScript property effects and diagnostic exceptions

6. Missing/null receiver handles fail at `this[kHandle].addAddress`; this is
   ordinary property access, not an explicit `ERR_INVALID_THIS` check. Both the
   initial and final address symbol reads, receiver symbol read, and eventual
   method call propagate abrupt completions. ECMA-262 [GetValue][getvalue]
   uses `? ToObject` and `Return ? ...[[Get]]`; [OrdinaryGet][ordinaryget]
   says `Return ? Call(_getter_, _receiver_)`. [Proxy [[Get]]][proxyget]
   validates non-revocation, invokes `? Call(_trap_, ...)`, and throws TypeError
   on violated invariants. These prove arbitrary-value propagation as well as
   the ordinary invalid receiver/revoked-proxy TypeErrors.
7. [`errors.js:995–1042,1456`][errors] builds the validation message using
   `determineSpecificType(actual)`. It reads function `.name`; for objects it
   evaluates `value.constructor && 'name' in value.constructor`, then interpolates
   `value.constructor.name`. Property access, `in` on nonobjects, proxy traps,
   and interpolation conversions can fail. Otherwise it calls
   `inspect(value, { depth: -1 })`. None is a successful conversion of the
   argument into a valid address; these are effects **inside** the callable's
   error reporting. Getter/hook repros assert supplied-value identity.
8. An ESM namespace with an uninitialized exported `const constructor` passes
   the symbol lookup but fails the diagnostic `.constructor` access. ECMA-262
   [Module namespace [[Get]]][namespaceget] returns
   `? _targetEnv_.GetBindingValue(...)`, and [Module GetBindingValue][binding]
   specifies `If the binding ... is an uninitialized binding, throw a
   ReferenceError exception.` Matching V8 [`objects/module.cc:405–425`][module]
   constructs `NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name)`.
   The fixture passes the already-created namespace as the argument; it does
   not evaluate the uninitialized binding before calling addAddress.
9. [`inspect.js:354–409,657–714,857–932`][inspect] uses default inspection
   options plus depth -1, reads the custom inspection symbol, then executes
   `FunctionPrototypeCall(maybeCustom, context, ...)`. The return is inspected
   again: `if (ret !== context) ... return formatValue(ctx, ret, recurseTimes);`.
   Two objects whose hooks return each other therefore recurse without either
   hook itself throwing or recursively invoking anything. The depth cutoff and
   `seen` handling occur later, so they do not stop this cycle. Matching V8
   [`execution/isolate.cc:1897–1911`][stack] constructs the exception using
   `range_error_function()` and `MessageTemplate::kStackOverflow`.
   The later `formatRaw` stack-overflow catch (1182–1185) does not surround
   this custom-return recursion. Prototype/constructor inspection, built-in
   type formatting and custom returned objects can also execute property/trap
   effects; these stay in the parametric `E`, never a finite list of errors.
10. Inspection usually unwraps proxies, but one layer is obtained by
    `getProxyDetails` (`inspect.js:868–881`). The custom-return repro supplies
    two nested proxies, leaving the inner proxy for key discovery.
    [`inspect.js:800–825`][inspect] first obtains own symbols, then runs
    `ObjectKeys(value)` in a try/catch. The catch asserts
    `isNativeError(err) && err.name === 'ReferenceError' && isModuleNamespaceObject(value)`.
    A second-ownKeys-call sentinel fails that assertion.
    [`internal/assert.js:8–12`][assert] throws `new ERR_INTERNAL_ASSERTION(message)`;
    [`errors.js:1377–1383`][errors] registers this as `Error`.
    This is a genuine new Error replacing the input trap exception, not identity
    propagation. Other uncaught custom hook/property errors remain arbitrary.
    Inspection's ignored `Symbol.toStringTag` getter errors (950–954), supported
    ESM key-discovery catch, and later stack-overflow rendering catch must not
    be described as universally propagating every inspection error.
11. Validators import the `HideStackFramesError` variants
    (`validators.js:24–34`). [`errors.js:407–427,536–547`][errors] sets
    `Error.stackTraceLimit = 0` before `super(...args)` and restores it only
    after successful construction. A diagnostic failure can therefore leave
    the limit at zero and skip the wrapper's conditional captureStackTrace.
    Tests restore the original limit in `finally` after **each** invocation;
    the ReferenceError child has its own process. Node's `.constructor` getter
    deliberately returns the base constructor (`errors.js:480–483`), so tests
    check that exact constructor **and** code, rather than accepting subclasses
    through a broad `toThrow(Error)` assertion.

## Mechanism review and current repros

The propagation repro now identifies both distinct private `kHandle` symbols from
real instances: `address[kSocketAddressHandle]` is read for branding and again for
the native argument; `this[kBlockListHandle]` is read for dispatch. The second-read
case counts only the address symbol. Diagnostic `constructor`/`constructor.name`
getters and invalid-family custom inspection remain separate dependencies. This
replaces unidentified first-read traps without mutating the native handles.
Proxies retain their declared target shape, so private-symbol reads are not proof
of effect-free typed receivers. Existing source/validation proof and test count
remain applicable; latest results are in the run checkpoint.

## Original audit validation (historical)

All seven tests use `{ tags: ["node-only"] }`. Required project: `node-24` only;
Chromium has no `node:net` implementation and is not selected for Node inventory.
Two `throws TypeError` cases cover validation and receiver/diagnostic branches;
two `throws Error` cases independently cover native parsing and replacement
assertion errors. Separate RangeError and ReferenceError repros establish their
constructors; `throws non-Error value` checks identity at six propagation sites.
Successful default/case-folded string and SocketAddress controls verify overload
dispatch and ignored family, not a no-errors inference.

From repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/net.test.ts \
  -t '^node:net > BlockList > addAddress > throws ' \
  --maxWorkers 1 --reporter verbose
```

Actual runtime: Node **24.19.0**, V8 **13.6.233.17-node.51**, libuv **1.52.1**,
Linux; Vitest **5.0.5**. The command selects precisely this file and callable
boundary (there are no regex metacharacters in these describe segments).
Final result: **1 file passed; 7 tests passed; 66 unrelated TODOs skipped**.
This includes the real ESM child on `process.execPath`, with an exact
ReferenceError assertion before printing `ReferenceError verified`.
Temporary fixture directory removal is unconditional in `finally`; no sockets
or other resources remain open. No mocks/polyfills or shared runner edits.

An initial run was **6 passed / 1 failed** because an extra diagnostic assertion
expected `determineSpecificType` in the child's stack. Node's hidden-stack
constructor produces only the ReferenceError message on this failure path.
The exact constructor assertion already passed. Replacing that unsupported
stack-frame assertion with the exact TDZ message yielded 7/7; after adding
stackTraceLimit restoration, the same filtered command again passed 7/7.
Exploratory `mise exec -- node --input-type=module` commands confirmed the
custom-return cycle/replacement branches; they are not substitutes for the
final filtered project run. Plain `node` outside mise was 22.23.1 and mise in
the Node source clone selected 26.12.1; neither was used for validation claims.

Final hygiene checks passed: `git diff --check -- pkgs/lib/tests/node/24/net.test.ts`
and `mise exec -- pnpm exec oxfmt --check pkgs/lib/tests/node/24/net.test.ts
'pkgs/lib/docs/evidence/node/24/net/node%3Anet#blocklist#addaddress.md'`.
An earlier formatting check reported layout-only differences; those were fixed
only inside the owned suite and evidence. The post-format filtered test run
again passed all seven repros.

## Delivery

No runtime/configuration blocker. Node declaration annotation delivery is
deferred to the coordinator; only the selected suite/imports and this evidence
file were changed. No declaration annotation table or inventory was modified.

[version]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27
[v8version]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/include/v8-version.h#L11-L14
[net]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/net.js#L2722-L2728
[blocklist]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/blocklist.js#L73-L83
[socketaddress]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/socketaddress.js#L45-L85
[validators]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L164-L168
[errors]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js
[transfer]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/worker/js_transferable.js#L91-L98
[sockaddr-inl]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_sockaddr-inl.h#L40-L49
[sockaddr]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_sockaddr.cc
[native-errors]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h
[uv-common]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/uv-common.c#L254-L297
[inet]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/inet.c#L146-L297
[unwrap]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/base_object.h#L249-L254
[module]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L425
[inspect]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js
[assert]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12
[stack]: https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1897-L1911
[getvalue]: https://tc39.es/ecma262/#sec-getvalue
[ordinaryget]: https://tc39.es/ecma262/#sec-ordinaryget
[proxyget]: https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver
[namespaceget]: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-get-p-receiver
[binding]: https://tc39.es/ecma262/#sec-module-environment-records-getbindingvalue-n-s
