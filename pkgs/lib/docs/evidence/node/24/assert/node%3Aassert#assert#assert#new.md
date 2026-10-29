# node:assert > assert > Assert > new

- ID: `node/24/assert.test.ts::["node:assert","assert","Assert","new"]`
- Test: `pkgs/lib/tests/node/24/assert.test.ts`, exact managed path above.
- Declarations: `@types/node@24.19.2/assert.d.ts:35–60,65–131`; `Assert` is a constructor-valued variable, not the `Assert` instance interface. Both overloads are covered: `new(options?: AssertOptions & { strict?: true | undefined }): AssertStrict` and `new(options: AssertOptions): Assert` (125–130). `AssertOptions` has `diff`, `strict`, and `skipPrototype`. The interfaces select assertion methods, not additional constructor implementations.
- Runtime mapping: exported `node:assert.Assert` → `function Assert(options)` in Node `lib/assert.js:108–135`; the same function is exported as `assert.strict.Assert` (940–943). Assertion methods on the resulting instance are **not** invoked by construction.
- Status: complete (behavioral audit; no declaration annotations delivered).
- Contract candidate: preserve each overload's success type and use `Returns<AssertStrict, unknown>` / `Returns<Assert, unknown>` conservatively. More precise error component: intrinsic `TypeError | ReferenceError | RangeError | Error` plus unknown/dependent effects from options copying, diagnostic inspection, and custom construction prototypes. These dependencies can throw any JavaScript value; the finite built-in list alone is unsound. `Throws<F>` takes a callable, not an error-class union.

## Conclusion

All errors described here are **synchronous construction/call effects**, including the tests whose fixtures require asynchronous module loading. No constructor-created promise, iteration-time failure, error-first callback, or emitted error event exists.

- `TypeError`: invalid non-undefined `diff` (`ERR_INVALID_ARG_VALUE`); calling without `new` (`ERR_CONSTRUCT_CALL_REQUIRED`, misuse outside the new signatures); revoked/invariant-violating options proxies; failed instance writes/property definition with supplied construction prototypes.
- `ReferenceError`: copying a module namespace whose exported binding is uninitialized. This happens inside `Object.assign`, not while evaluating the constructor argument.
- `RangeError`: invalid `diff` custom-inspection hooks can return one another's objects indefinitely. The hooks return normally; Node's diagnostic traversal itself overflows V8's stack.
- Exact base `Error`, code `ERR_INTERNAL_ASSERTION`: inspecting an invalid namespace-valued `diff` whose exported object has a throwing custom-inspection hook. Namespace formatting catches that throw, requires it to be a native `ReferenceError`, and creates an internal assertion error when it is not.
- Arbitrary values propagate from enumerable options getters (even unrelated properties), proxy traps, diagnostic hooks/property access, and supplied new-target/prototype accessors. The separate `SyntaxError` repro is **propagation**, not evidence of an intrinsic syntax parser in `Assert`.

Scope: Node 24.19.0 with ordinary, unmodified primordials and `util.inspect` defaults; no monkey-patching or process startup snapshot construction. Resource exhaustion/process termination is excluded. The stack-overflow case is specifically the callable's recursive diagnostic algorithm, not an already-exhausted caller stack. Other Node releases need rechecking. No OS/platform branch occurs in this constructor. Changing global inspection/stack-trace configuration or supplying additional native/user inspection hooks adds those supplied behaviors to the dependent effect, not to a claimed finite list.

## Proof

### Revisions and mapping

Local Node checkout HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, **not** the runtime revision. Every Node source citation below was read using `git show v24.19.0:<path>`, whose peeled commit is **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**. Its [`src/node_version.h:25–33`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L33) declares `24`, `19`, `0`; bundled [`deps/v8/include/v8-version.h:11–14`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/include/v8-version.h#L11-L14) declares `13.6.233.17`. Runtime reports Node **v24.19.0**, V8 **13.6.233.17-node.51**.

ECMA-262 local revision: **`40ce6f206b5669e69bf010a38267ae24a2843404`**. Spec links below name algorithm anchors; [pinned spec text](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html) supplies the recorded revision.

### Constructor and options copying

[`lib/assert.js:108–135`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L108-L135): `if (!new.target)` throws `new ERR_CONSTRUCT_CALL_REQUIRED('Assert')`. Then:

> `options = ObjectAssign({ __proto__: null, strict: true, skipPrototype: false }, options);`

The fresh, writable null-prototype destination makes `__proto__` an ordinary copied key, not a prototype setter. Null/undefined sources are skipped; other primitives are boxed, not rejected merely for being primitive. No option values undergo string or numeric coercion here. All own enumerable keys, including symbols/unrecognized keys, are copied. Inherited/non-enumerable source properties are not copied.

[Object.assign](https://tc39.es/ecma262/#sec-object.assign), pinned spec lines 31158–31175, explicitly uses `? ...[[OwnPropertyKeys]]()`, `? ...[[GetOwnProperty]]`, `? Get`, and `? Set`. Those abrupt completions are not caught by `Assert`. [Proxy ownKeys](https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-ownpropertykeys) checks revocation, callable traps, array-like keys, duplicates and target invariants; get-own-property/get checks likewise reject invalid trap results/invariants. These create `TypeError` or propagate trap/getter exceptions. There is no iterable consumption.

[Module namespace [[GetOwnProperty]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-getownproperty-p), spec 15645–15661, obtains its descriptor value using `? _obj_.[[Get]](...)`. [Namespace [[Get]]](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects-get-p-receiver), 15706–15732, delegates to module bindings. [Module GetBindingValue](https://tc39.es/ecma262/#sec-module-environment-records-getbindingvalue-n-s), 11712–11736: “If the binding ... is an uninitialized binding, throw a *ReferenceError* exception.” Matching V8 [`deps/v8/src/objects/module.cc:405–445`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/objects/module.cc#L405-L445) creates `NewReferenceError` for hole-valued namespace exports; descriptor lookup uses `MessageTemplate::kNotDefined`. The self-import fixture catches the error around `new Assert(options)` before the export's initializer runs.

### Diff validation and its diagnostic effects

`assert.js:115–118` validates only a non-undefined `options.diff`, against `['simple', 'full']`. [`lib/internal/validators.js:24–34,210–219`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/validators.js#L210-L219): `if (!ArrayPrototypeIncludes(oneOf, value))` leads to `throw new ERR_INVALID_ARG_VALUE(name, value, reason)`. The allowed list and reason construction are fixed primitive strings; membership testing does not coerce the supplied value. The validator imports the `HideStackFramesError` variant (27).

[`lib/internal/errors.js:1460–1467`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1460-L1467):

> `let inspected = lazyInternalUtilInspect().inspect(value);`

The registration's primary class is `TypeError`; its optional `.RangeError` variant is **not** selected by this validator. `ERR_CONSTRUCT_CALL_REQUIRED` is also registered with `TypeError` (1153). Registration/factories at [407–428,464–493,551–582](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L407-L493) produce coded subclasses whose public `constructor` getter deliberately returns the base constructor (`return clazz` / `return Base`). Tests check this precise public constructor, name, and codes, rather than relying on `instanceof Error`.

Diagnostic inspection is a real additional execution path, not inert stringification:

- [`lib/internal/util/inspect.js:354–374,405–409`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L354-L409) selects inspection defaults and calls `formatValue(ctx, value, 0)`.
- [857–932](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L857-L932): primitives/null format directly; proxies are unwrapped using engine details (revoked proxies print a marker, unlike revoked **options** proxies). `const maybeCustom = value[customInspectSymbol]` can run a getter; `FunctionPrototypeCall(maybeCustom, ...)` invokes the hook. For a non-string return other than the original context: `return formatValue(ctx, ret, recurseTimes)`. There is no depth decrement, cycle registration, or catch on that return path. A two-object hook-return cycle therefore overflows. Matching V8 [`deps/v8/src/execution/isolate.cc:1862–1913`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/src/execution/isolate.cc#L1862-L1913) uses `range_error_function()` and `MessageTemplate::kStackOverflow`, then `Throw(*exception)`.
- Non-custom object formatting dispatches through [935–1186](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L935-L1186). Prototype traversal (`getConstructorName`, 657–713), constructor-name/iterator/regexp/error/URL property reads, and nested values can invoke supplied accessors or hooks; their effects remain dependent, not a finite error list. Dates are checked for NaN before ISO conversion (1078–1082); a null-prototype genuine RegExp is cloned using its existing internal pattern (1066–1069), not parsed from arbitrary user source text (ECMA-262 [RegExp constructor](https://tc39.es/ecma262/#sec-regexp-pattern-flags), 39476–39486). Thus these branches do not justify an intrinsic invalid-date `RangeError` or regexp `SyntaxError`. Ordinary property getters are not evaluated with default inspection options; explicit inspection-hook reads and special-property reads still are. Formatting catches some failures (`Symbol.toStringTag`, stack formatting, ordinary traversal stack overflow), so not every getter failure necessarily escapes.
- Namespace formatting is selected at 1121–1124. [2003–2021](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L2003-L2021) catches a failure from `formatProperty` and executes `assert(isNativeError(err) && err.name === 'ReferenceError')`. A primitive thrown by an exported object's custom hook fails this condition. [`lib/internal/assert.js:8–12`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/assert.js#L8-L12) throws `new ERR_INTERNAL_ASSERTION(message)`; [`errors.js:1377–1383`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1377-L1383) registers it with exact base `Error`. This replaces, rather than propagates, that hook's thrown primitive.
- [`errors.js:407–425`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L407-L425) sets `Error.stackTraceLimit = 0` before `super(...args)` and restores it only on normal completion. If inspection throws, it remains zero. The `hideStackFrames` wrapper ([536–543](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L536-L543)) consequently skips recapturing the stack and rethrows the value, including primitives. Repros restore the original stack limit in `finally`; no global configuration is changed to force the behavior. A non-writable/custom stack limit is outside the ordinary-global assumption and can affect error recapture.

### Instance initialization / supplied construction prototype

The remaining constructor body assigns `this.AssertionError`, defines a non-enumerable non-configurable options symbol, and, when `options.strict` is truthy, copies four strict methods to their loose aliases (`assert.js:120–134`). `strict` uses truthiness, not user coercion, and `skipPrototype` is only stored. Invalid booleans do not trigger a boolean validator. With the original constructor/prototype, all destination writes are ordinary and safe; no assertion method is called. With a supplied `newTarget`, inherited setters/getters, non-writable properties, or proxy prototypes can affect these operations.

ECMA-262 [function [[Construct]]](https://tc39.es/ecma262/#sec-ecmascript-function-objects-construct-argumentslist-newtarget), [OrdinaryCreateFromConstructor](https://tc39.es/ecma262/#sec-ordinarycreatefromconstructor), and [GetPrototypeFromConstructor](https://tc39.es/ecma262/#sec-getprototypefromconstructor) propagate `? Get(_ctor_, "prototype")`. The allocated receiver is still ordinary, not a supplied proxy receiver. [OrdinarySetWithOwnDescriptor](https://tc39.es/ecma262/#sec-ordinarysetwithowndescriptor), 13397–13431, delegates to inherited `[[Set]]`, returns false for non-writable data properties, or performs `? Call(_setter_, ...)`. [PutValue](https://tc39.es/ecma262/#sec-putvalue) turns unsuccessful strict-mode writes into `TypeError`. [Object.defineProperty](https://tc39.es/ecma262/#sec-object.defineproperty), 31227–31237, calls `? DefinePropertyOrThrow`; the test's local inherited setter makes the fresh receiver non-extensible before the options symbol is defined. No shared built-in prototype is modified.

## Mechanism review and current repros

The options-spread repro distinguishes `ownKeys`, `getOwnPropertyDescriptor("strict")`,
and `get("strict")`. The getter on an unrelated enumerable key is intentionally
retained: the dependency is `options.* (enumerable own keys)`, not only declared
option names. Such extra properties can exist on a structurally typed object.
Construction hooks now target `newTarget.prototype` explicitly, separately from
the instance `AssertionError` setter and `strictEqual` getter. Those reflective
newTarget conditions require their own unchecked construction assumptions.
Invalid `diff` representatives sharing one enum-validation branch were collapsed;
the diagnostic get/call, recursion and namespace mechanisms remain distinct.
Current results are in the run checkpoint; type assertions no longer check
diagnostic names/codes/messages.

## Original audit validation (historical)

All 13 cases carry `{ tags: ["node-only"] }` and run in `node-24` only:

| Case(s), in source order | Condition / assertion |
| --- | --- |
| `throws TypeError` ×5 | Invalid diff values/code; missing `new`/code; revoked proxy and duplicate ownKeys; inherited non-writable assignment; non-extensible instance definition. Exact constructor/name checked. |
| `throws Error` | Enumerable unrelated options getter; supplied Error identity, exact constructor. |
| `throws non-Error value` ×2 | Each options-copy trap; new-target prototype lookup plus inherited instance setter/method getter. Supplied object/symbol identity. |
| `throws SyntaxError` | Invalid diff's custom inspection throws supplied SyntaxError; identity and constructor. |
| `throws non-Error value` | Inspection-hook getter throws primitive 17; identity. |
| `throws RangeError` | Hooks return a two-object cycle; exact constructor/name/stack-overflow message. |
| `throws ReferenceError` | Native self-importing module passes its not-yet-initialized namespace to Assert; exact constructor/name. |
| `throws Error` | Namespace export inspection replaces primitive throw with exact Error/code `ERR_INTERNAL_ASSERTION`. |

The native-namespace fixture is a temporary `.mjs` file removed recursively in `finally`. `vm.runInThisContext` with `USE_MAIN_CONTEXT_DEFAULT_LOADER` loads it with Node's actual module semantics, without transpilation, mocks, polyfills, or extra Node flags. Node emits the expected ExperimentalWarning for that loader option. The data-URL fixture for internal assertion formatting also creates a real namespace. Fixture imports finish successfully; the constructor errors are caught at the constructor call itself.

Commands (repository root):

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/assert.test.ts \
  -t '^node:assert > assert > Assert > new > throws ' --maxWorkers 1
git diff --check -- pkgs/lib/tests/node/24/assert.test.ts
```

Final result: **1 file passed; 13 passed, 27 skipped (40 total)**, Vitest **5.0.5**, project **node-24**, Node **v24.19.0**, V8 **13.6.233.17-node.51**. Skips are other, untouched callable TODOs. Regex has the full managed suite and trailing test boundary; there are no regex metacharacters in this suite name requiring escaping. No Chromium project is required for this Node-only callable. Diff check passed.

Earlier executions are not counted as final validation: an initial 12-case run had 7 failures because `toHaveProperty("constructor", ...)` does not read that property in this assertion library; direct `.constructor` identity assertions fixed it (12/12 passed). Adding the namespace case first failed because Vitest transformed the file namespace, then failed only an overly specific message assertion after switching to the native loader. Final native run above passes all 13. Standalone `mise exec -- node --input-type=module` probes verified diagnostic-hook propagation, diagnostic recursion, and namespace internal assertion behavior before adding the tests.

## Open questions / annotation delivery

No unresolved behavioral or runner blocker under the stated runtime/global assumptions. The **Node annotation delivery gap remains**: declarations are external `@types/node`, with no durable Node declaration transform established here. The coordinator must integrate both constructor-valued overload contracts through a durable mechanism, including dependent/unknown effects; do not edit installed types or treat passing repros as annotation delivery. No config/helper/dependency changes are requested.
