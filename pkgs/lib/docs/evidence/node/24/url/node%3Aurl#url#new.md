# node:url > URL > new

- ID: `node/24/url.test.ts::["node:url","URL","new"]`
- Test: `pkgs/lib/tests/node/24/url.test.ts`, exact managed path above.
- Declaration: installed `@types/node@24.19.2`, `url.d.ts:539`:
  `constructor(input: string | { toString: () => string }, base?: string | URL)`.
  This is the single constructor overload. `node:url` re-exports `url`
  (`url.d.ts:1101–1103`); the global interface/conditional constructor alias at
  1077–1084 does not add an overload to this module class. `URLSearchParams` and
  `URLPattern` constructors are different inventory entries.
- Runtime mapping: `lib/url.js:57–70,1029–1040` exports the `URL` class from
  `lib/internal/url.js:806–852`; its native parser is `BindingData::Parse` in
  `src/node_url.cc`, backed by Ada `url_aggregator`.
- Status: complete.
- Contract candidate: `Returns<URL, TypeError | ReferenceError | Error | EInputCoercion | EBaseCoercion | EConstruction>`.
  These effect parameters mean the thrown values of invoked conversions/property
  access/construction hooks, not just subclasses of Error. Without effect
  constraints the conservative spelling is `Returns<URL, unknown>`. For primitive
  strings and ordinary construction it reduces to `Returns<URL, TypeError | Error>`.
  Node annotation delivery is deferred; no declarations have been annotated.

## Conclusion

All effects are **synchronous**. Intrinsic concrete constructors:

- **TypeError**: missing argument (`ERR_MISSING_ARGS`); invalid input or base
  (`ERR_INVALID_URL`); failed string-to-primitive conversion, symbol-to-string
  conversion, revoked/invariant-violating proxies, and calling the class without
  construction (uncoded engine TypeErrors).
- **Error**, not RangeError: native string-size rejection (`ERR_STRING_TOO_LONG`),
  including percent-encoded output exceeding V8's string limit. This is a normal
  checked length failure, not an OOM demonstration.
- **ReferenceError**: input/base coercion reads an uninitialized lexical export
  named `toString` or `valueOf` from a module namespace. This includes inherited
  namespaces and namespace descriptor reads during proxy invariant validation.
  These are engine-created errors during URL's invocation, not exceptions
  explicitly thrown by supplied conversion code. They have no Node error code.
- Arbitrary supplied values propagate from input/base `@@toPrimitive`, `toString`,
  `valueOf`, their getters/proxies, and `newTarget.prototype` access. Supplied Error
  and SyntaxError repros assert identity; neither implies native SyntaxError.

No promise, iterator, event, or error-first-callback channel is involved. Coercion
methods are called synchronously. No native SyntaxError is produced by invalid URL
syntax. A finite `TypeError | ReferenceError | Error` contract alone would still
lose non-Error effects. ReferenceError must not be hidden in the effect parameters
merely because it arises during coercion: the exotic lookup itself creates it.

Scope: Node 24.19.0, normal public API, unmodified built-ins/prototypes, sufficient
memory for deterministic length validation. Excludes process termination/OOM,
arbitrary monkey-patching, and exposing private bindings with internal flags.
The constructor does no filesystem/network I/O; file URLs are parsed, not opened.
There is no OS-specific public constructor dispatch. Internal path-construction
tokens are not public inputs.

## Proof

### Source identity and export resolution

All Node source citations below use revision
[`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`](https://github.com/nodejs/node/tree/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae).
Local Node clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, so research used
`git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:<path>`,
not the checkout's working files. The pinned
[`src/node_version.h:25–27`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27)
defines 24/19/0; `deps/v8/include/v8-version.h:11–14` defines 13/6/233/17;
`deps/ada/ada.h:11271–11278` defines Ada 3.4.4. They match the tested runtime:
Node **24.19.0**, V8 **13.6.233.17-node.51**, Ada **3.4.4**, Linux x64.

[`lib/url.js:57–70,1037–1040`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/url.js#L57-L70)
states “WHATWG URL implementation provided by internal/url” and exports that
`URL`. It is an actual JavaScript class, not a callback/interface placeholder.

### Initialization, arity, and conversions

[`lib/internal/url.js:796–852`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/url.js#L796-L852):
“`if (arguments.length === 0)`” leads to “`throw new ERR_MISSING_ARGS('url');`”.
An explicitly supplied undefined instead becomes `"undefined"`, then undergoes
parsing. The error registration ends in `TypeError` at
[`lib/internal/errors.js:1588–1602`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1588-L1602).

The constructor executes “``input = `${input}`;``” followed, only when base is not
undefined, by “``base = `${base}`;``”. Input conversion precedes base conversion;
base conversion and validation occur even when input is absolute. There is no catch.

ECMA-262 research clone revision:
[`40ce6f206b5669e69bf010a38267ae24a2843404`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html).
The following stable algorithm anchors describe the JavaScript operations used
by the matching Node implementation:

- [Template Evaluation](https://tc39.es/ecma262/#sec-template-literals-runtime-semantics-evaluation),
  `spec.html:19463–19478`: “Let _middle_ be ? ToString(_sub_).”
- [ToString](https://tc39.es/ecma262/#sec-tostring), lines 5653–5677:
  “If _arg_ is a Symbol, throw a *TypeError* exception”; object conversion uses
  “? ToPrimitive(_arg_, ~string~)”.
- [ToPrimitive](https://tc39.es/ecma262/#sec-toprimitive) and
  [OrdinaryToPrimitive](https://tc39.es/ecma262/#sec-ordinarytoprimitive),
  lines 5027–5082: `GetMethod`/`Call` of `@@toPrimitive` propagate abruptly;
  an object result throws TypeError. Ordinary conversion gets/calls `toString`
  then `valueOf`, propagating each abrupt completion and throwing TypeError if
  neither returns a primitive.
- [GetMethod](https://tc39.es/ecma262/#sec-getmethod), lines 6341–6357:
  “If IsCallable(_func_) is *false*, throw a *TypeError* exception.” Accessor
  exceptions propagate through `? GetV` before this check.
- [Proxy [[Get]]](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver),
  lines 16292–16318: `? ValidateNonRevokedProxy`, `? GetMethod`, and `? Call`
  preserve thrown values; invariant violations explicitly throw TypeError.

### Exotic property access: intrinsic ReferenceError, not just supplied effects

The original audit omitted the module-namespace branch. The reviewed result adds
ReferenceError explicitly. A native ESM self-import has a fully initialized
namespace binding while its exported lexical `toString` binding can still be in
the temporal dead zone. Passing the namespace as an argument does not read that
export. Inside URL, template coercion finds no `@@toPrimitive`, then
OrdinaryToPrimitive performs `? Get(namespace, "toString")` (or, after a
non-primitive `toString` result, `? Get(namespace, "valueOf")`).

At the ECMA-262 revision pinned above:

- [Module namespace [[Get]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-get-p-receiver),
  `spec.html:15706–15732`, uses ordinary lookup for symbols, resolves string export
  names, and returns “? _targetEnv_.GetBindingValue(_binding_.[[BindingName]], *true*)”.
  It also explicitly throws ReferenceError for an empty target environment.
- [Module environment GetBindingValue](https://tc39.es/ecma262/#sec-module-environment-records-getbindingvalue-n-s),
  lines 11712–11735: “If the binding for _name_ in _envRecord_ is an uninitialized
  binding, throw a *ReferenceError* exception.” Re-export indirection recursively
  delegates to the target environment with abrupt-completion propagation.
- [OrdinaryGet](https://tc39.es/ecma262/#sec-ordinaryget), lines 13337–13360,
  propagates both `? _obj_.[[GetOwnProperty]]` and
  “Return ? _parent_.[[Get]](_propertyKey_, _receiver_)”. Thus a namespace inherited
  by an otherwise ordinary argument gives the same ReferenceError. An exotic
  object on a constructible newTarget's property lookup chain likewise cannot
  be treated as infallible by GetPrototypeFromConstructor.
- [Module namespace [[GetOwnProperty]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-getownproperty-p),
  lines 15645–15662: “Let _value_ be ? _obj_.[[Get]](_propertyKey_, _obj_).” Proxy
  [[Get]] reads this descriptor **after** its trap returns normally. Therefore a
  proxy over a TDZ namespace can throw native ReferenceError even when its get
  trap returns a perfectly usable conversion method and never throws itself.
  Proxy revocation/invariant violations instead create the already-listed
  TypeError. The updated repro covers both kinds of native proxy validation.

Matching Node/V8 implementation corroborates both paths:
[`deps/v8/src/builtins/accessors.cc:242–252`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/builtins/accessors.cc#L242-L252)
calls `holder->GetExport(...)` from `ModuleNamespaceEntryGetter`.
[`deps/v8/src/objects/module.cc:405–445`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L445)
checks `IsTheHole(*value, isolate)` and executes
“`NewReferenceError(MessageTemplate::kAccessedUninitializedVariable, name)`”
in `GetExport`; `GetPropertyAttributes` independently throws
“`NewReferenceError(MessageTemplate::kNotDefined, name)`” for the same uninitialized
cell. This explains why messages can differ while the exact constructor is the
same; the test checks constructor/name and absence of a Node code, not wording.

Other ECMAScript exotic *property lookup* paths were separated from calls of the
retrieved conversion methods rather than assigned blanket unknown effects:
[Arrays](https://tc39.es/ecma262/#sec-array-exotic-objects) use ordinary essential
methods except [[DefineOwnProperty]] (`spec.html:14598–14603`);
[StringGetOwnProperty](https://tc39.es/ecma262/#sec-stringgetownproperty)
(14834–14855) returns undefined for these non-index hook names/symbols;
[mapped arguments [[Get]]](https://tc39.es/ecma262/#sec-arguments-exotic-objects-get-p-receiver)
(14939–14956) delegates unmapped hook names to OrdinaryGet; and
[TypedArray [[Get]]](https://tc39.es/ecma262/#sec-typedarray-get) (15204–15222)
delegates non-numeric names/symbols to OrdinaryGet. They add no distinct error
constructor merely by looking up `@@toPrimitive`, `toString`, `valueOf`, or
`prototype`. Their inherited accessors, proxies, namespace prototypes, and invoked
conversion methods still follow the respective branches above. Effects of a
supplied callable (including a supplied built-in method with its own validation)
remain dependent effects; native namespace and proxy lookup errors are explicitly
enumerated rather than dismissed as such supplied-code effects.

### Native parser: all failures converge, not SyntaxError

[`src/node_url.cc:384–426`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_url.cc#L384-L426)
receives already-converted strings. It parses base first if present:
“`if (!base && raise_exception)`” calls `ThrowInvalidURL`. Input parsing likewise
uses “`if (!out && raise_exception)`” and calls that same routine.
`Utf8Value` uses `MakeUtf8String`
([`src/util.cc:99–138`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util.cc#L99-L138));
the input is already a primitive string, and UTF-8 writing replaces invalid UTF-16
sequences rather than invoking supplied code or throwing a URIError.

[`deps/ada/ada.cpp:11127–11135`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/ada/ada.cpp#L11127-L11135):
`parse_url_impl` returns a URL record; “`if (!u.is_valid)`” returns
“`tl::unexpected(errors::type_error)`”. The parser state machine at 13211–14123
handles scheme/no-scheme, authority, relative/special URL, host/port, path/query/
fragment, opaque and file paths. Failures return invalid records, not JS errors:

- No usable base / opaque base with relative path: 13383–13429,
  “`url.is_valid = false; return url;`”.
- Empty authority after credentials or missing special host: 13535–13539,
  13764–13815. Invalid host, IPv4/IPv6, IDNA or forbidden domain characters:
  `parse_host` 14625–14728, IPv4 15100–15194, IPv6 15196–15435; validation
  consistently uses “`return is_valid = false;`”.
- Port overflow/trailing invalid content: parser 13847–13854 and
  [`deps/ada/ada.h:8705–8741`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/ada/ada.h#L8705-L8741),
  “`if (r.ec == std::errc::result_out_of_range)`” sets `is_valid = false`.
- File host parsing calls the same host validator (13984–14028); file/Windows
  drive-letter handling is URL grammar, not an OS call. Path, userinfo, query and
  fragment encoding assemble the record; all returns still pass through the
  common `is_valid` and native output conversion checks.

[`src/node_url.cc:561–587`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_url.cc#L561-L587)
creates “`ERR_INVALID_URL(env->isolate(), "Invalid URL")`”, attaches the converted
`input`/optional `base`, then `ThrowException(err)`. The native error table explicitly
maps `ERR_INVALID_URL` to **TypeError**
([`src/node_errors.h:105`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h#L105)).

For fewer than three arguments, `raiseException` is true. With extra arguments,
`parseSymbol !== kParseURLSymbol` is still true for every public value. The three
tokens at `internal/url.js:802–804` are fresh, inaccessible symbols: a same-description
symbol is unequal. Thus the private null-on-failure / pathToFileURL branches are
not reachable through this public constructor's arguments.

### Native string limit: intrinsic Error

Successful parsing converts `out->get_href()` via `ToV8Value` (`node_url.cc:421–424`).
[`src/util-inl.h:344–357`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util-inl.h#L344-L357)
checks “`str.size() >= static_cast<size_t>(v8::String::kMaxLength)`” and calls
`ThrowErrStringTooLong`. That function executes
“`isolate->ThrowException(ERR_STRING_TOO_LONG(isolate));`”
([`src/util.cc:217–219`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util.cc#L217-L219)).
[`src/node_errors.h:125,306–311`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h#L125)
maps it to **Error**, with “Cannot create a string longer than ... characters”.
The same conversion while attaching invalid-URL `input`/`base` can supersede the
pending TypeError; `ThrowInvalidURL` explicitly comments “A superseding error has
been thrown.” This adds the same Error type, not another channel.

Ada's path encoder (`ada.cpp:13904–13924,15630ff`) performs percent encoding;
the encoding primitive at 10942–10965 appends three output characters per encoded
byte: “`result.append(character_sets::hex + uint8_t(*pointer) * 4, 3);`”.
U+0800 occupies three UTF-8 bytes, hence nine ASCII output characters. The repro
builds only 59,652,342 input UTF-16 units, below `MAX_STRING_LENGTH = 536870888`,
but produces a serialized URL exceeding that limit. Fixture creation is outside
the caught call, so this proves URL's native conversion error, not String.repeat's
RangeError. A subprocess releases the large allocation immediately.

### Construction hooks and successful completion

[ECMAScript [[Construct]]](https://tc39.es/ecma262/#sec-ecmascript-function-objects-construct-argumentslist-newtarget),
`spec.html:13944–13978`, calls `? OrdinaryCreateFromConstructor` for this base class,
initializes fields, and propagates the body result. [GetPrototypeFromConstructor](https://tc39.es/ecma262/#sec-getprototypefromconstructor),
lines 13546–13565, uses “Let _proto_ be ? Get(_ctor_, *"prototype"*).” Consequently
a `Reflect.construct(URL, args, newTarget)` prototype getter/proxy failure is
inside URL's construction operation, not argument evaluation. The fixture
newTarget's body is never invoked. Revoked proxies also fail here; fallback
[GetFunctionRealm](https://tc39.es/ecma262/#sec-getfunctionrealm) propagates proxy
revocation. [Class [[Call]]](https://tc39.es/ecma262/#sec-ecmascript-function-objects-call-thisargument-argumentslist)
rejects calling a class without `new` with TypeError; it cannot initialize an
arbitrary supplied receiver.

The fresh instance gets private context/flags (`internal/url.js:806–809`);
`URLContext` fields at 176–204 are ordinary own data. `markTransferMode`
([`lib/internal/worker/js_transferable.js:91–98`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/worker/js_transferable.js#L91-L98))
writes an internal private symbol, not a public setter. Matching V8 lookup bypasses
proxy traps for private symbols
([`deps/v8/src/objects/lookup.cc:1280–1300`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/lookup.cc#L1280-L1300)).
After parsing, `#updateContext` (`internal/url.js:916–958`) writes the fresh internal
context from native component data. `#searchParams` is initially undefined, so its
lazy-update branch is not reached by construction. No user property/callback is
read on this successful tail, and no deferred work is scheduled.

## Mechanism review and current repros

The input/base proxy repro now targets `Symbol.toPrimitive` retrieval rather than
an unidentified first read. Ordinary `toString` invocation, exotic primitive-hook
invocation, getter/coercion fallbacks, and `newTarget.prototype` access remain
distinct dependencies. Input/base position and parser/native-string-limit errors
remain covered; successful coercion setup stays outside capture. Current tests
use exact types/identity rather than redundant diagnostic payload checks. Counts
and latest validation are in the run checkpoint.

## Original audit validation (historical)

Required project: **node-24 only** (Node inventory, coordinator constraint).
Vitest **5.0.5**; runtime versions above were obtained using:

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- node -p 'JSON.stringify(process.versions)'
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise run //pkgs/lib:test/node tests/node/24/url.test.ts -t '^node:url > URL > new > throws ' --maxWorkers 1 --reporter verbose
```

The regex contains the exact full suite plus trailing test boundary; there are
no regex metacharacters in these describe names. No other callable runs.
Twelve tests execute: six `throws TypeError` cases (arity, input parser table,
base/private-token branches, symbols, primitive conversion failures, revoked
proxies/invariant violations/no-new); one `throws ReferenceError` case (native ESM,
eight branches); two `throws Error` cases (native limit in a same-binary child,
supplied coercion Error); one `throws SyntaxError` (supplied primitive hook);
two `throws non-Error value` cases (input/base getters/methods/proxy and
newTarget prototype access). Every intrinsic assertion checks exact constructor
identity, with stable codes where present. Propagation tests check supplied-value
identity. All are synchronous, including the child URL call.

Initial run: 6 failed / 5 passed because Vitest's property-path matcher did not
resolve `constructor`; direct `.constructor` identity assertions fixed the test
assertions without changing the runtime. Next run: **11 passed, 35 skipped**
(the selected TODO was still present). After removing only that TODO, the original
run was **11 passed, 34 skipped**, one file, node-24. Review correction reran the
same exact command: **12 passed, 34 skipped**, one file, node-24. Actual runtime
was rechecked as Node 24.19.0 / V8 13.6.233.17-node.51 / Ada 3.4.4.

The ReferenceError test writes a temporary `self.mjs`, launches the same Node
binary directly (no Vite/module emulation), and synchronously asserts exact
ReferenceError in eight URL invocations: namespace `toString` TDZ, inherited
namespace TDZ, proxy descriptor validation TDZ, and namespace `valueOf` TDZ,
each as input and as base. Once both lexical exports are initialized, successful
construction verifies the fixture itself is valid. The child reports the branch
count/version, asserted by the parent. Module evaluation itself completes
successfully; these are not import-promise rejections. `mkdtempSync` with the
platform temp directory and `rmSync` in `finally` provide portable fixture cleanup.
The large native-limit repro completes in about one second; child exit/timeout
cleanup owns its memory. No sockets, network access, global mutations, mocks, or
polyfills are used.

Formatting was scoped to the owned test file:
`PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- pnpm exec oxfmt pkgs/lib/tests/node/24/url.test.ts`.
`git diff --check -- pkgs/lib/tests/node/24/url.test.ts 'pkgs/lib/docs/evidence/node/24/url/node%3Aurl#url#new.md'`
passed; the test diff changes only imports and the selected callable suite.
No shared runner/dependency changes or annotation-generation steps were needed.
