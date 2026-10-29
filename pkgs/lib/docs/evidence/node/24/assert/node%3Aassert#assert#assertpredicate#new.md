# node:assert > assert > AssertPredicate > new

- ID: `node/24/assert.test.ts::["node:assert","assert","AssertPredicate","new"]`
- Test: `pkgs/lib/tests/node/24/assert.test.ts:196–204`; only the nested `new` suite is owned by this audit. Its literal TODO is retained.
- Declaration: `node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/assert.d.ts:333`; one construct signature, `new() => object`, inside the `AssertPredicate` union. There are no constructor overloads for this member.
- Runtime mapping: **none**. This is a declaration-only type for a supplied constructor, not `assert.Assert`, `assert.AssertionError`, or a runtime export named `AssertPredicate`.
- Status: **blocked** (definitive declaration-only mapping blocker).
- Contract candidate: supplied-constructor-dependent `new() => Returns<object, unknown>` as a conservative conceptual effect, not a proposed finite built-in error annotation. No annotation was delivered.

## Conclusion

No intrinsic Node error set can be assigned to this entry: there is no unique implementation to audit. This does **not** prove `never`. A caller-selected constructor can synchronously throw arbitrary JavaScript values through its body, initialization, or construction hooks. The signature imposes no error constraint. Rejection, iteration, event, and callback delivery are not specified by this constructor type; any such effects depend on the supplied implementation and returned object.

Testing construction of `assert.AssertPredicate` would test misuse of an absent property, not this signature. Constructing a locally invented class, `Assert`, or `AssertionError` would test a different callable. Neither can justify replacing the TODO. Errors in `throws`, `rejects`, `doesNotThrow`, or `doesNotReject` also belong to their own callable audits, not to this type member.

## Proof

### Declaration and alias resolution

Primary published declaration: [`@types/node@24.19.2/assert.d.ts`](https://unpkg.com/@types/node@24.19.2/assert.d.ts), local SHA-256 `4ea15fd99b2e34cb25fe8346c955000bb70c8b423ae4377a972ef46bfb37f595`.

- Lines 6–15 declare module `"assert"`, its function, and merged namespace `assert`.
- Line 333 states exactly: `type AssertPredicate = RegExp | (new() => object) | ((thrown: unknown) => boolean) | object | Error;`. The managed `new` path selects the structural constructor constituent, not the neighboring callback constituent or a value declaration. Neither the alias nor its constructor signature supplies executable code.
- Lines 830, 894, 992–996, and 1043–1047 use `AssertPredicate` as the `error` argument of `throws`, `doesNotThrow`, `rejects`, and `doesNotReject`, respectively. These are consumers of supplied values, not implementations of the constructor signature.
- Lines 1110–1115 use `export = assert`, then `import assert = require("assert"); export = assert;` in module `"node:assert"`. The module alias does not turn the namespace's type alias into a runtime property.

### Matching Node implementation

Local Node clone HEAD: `510d84c330e8e7d2bb8699608163024baf54eaa6` (not the tested release). Sources were read with `git show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:<path>`. Tag `v24.19.0` peels to **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**; [`src/node_version.h:25–33`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L33) declares major `24`, minor `19`, patch `0`.

- [`lib/assert.js:89`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L89): `const assert = module.exports = ok;`. Export setup at [922–943](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L922-L943) copies assertion methods and assigns `assert.Assert = Assert;`, not `AssertPredicate`.
- [`expectedException`, lines 500–579](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L500-L579) distinguishes regex/validation objects from supplied functions. Its constructor-like match is `expected.prototype !== undefined && actual instanceof expected`. Otherwise it recognizes Error subclasses or uses `const res = FunctionPrototypeCall(expected, {}, actual);`. It does not instantiate a Node-defined `AssertPredicate` constructor.
- [`hasMatchingError`, lines 706–723](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/assert.js#L706-L723) similarly uses `actual instanceof expected` and `FunctionPrototypeCall(expected, {}, actual)`. These consumers can have matching/property/hook effects, but they do not identify any unique implementation for the selected `new()` type member.

### Why an empty or finite error set is unjustified

ECMA-262 local revision **`40ce6f206b5669e69bf010a38267ae24a2843404`**. [Construct](https://tc39.es/ecma262/#sec-construct), [pinned text lines 6413–6428](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L6413-L6428), says: `Return ? ctor.[[Construct]](argList, newTarget).` The chosen constructor determines the operation and its abrupt completion. [ECMAScript function [[Construct]]](https://tc39.es/ecma262/#sec-ecmascript-function-objects-construct-argumentslist-newtarget), [pinned lines 13944–13978](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13944-L13978), evaluates the supplied body and says: `If result is a throw completion, return ? result.` Initialization failures likewise propagate. A structural signature provides no body or hook constraints from which to derive an exhaustive concrete error list. This spec citation explains the dependent effect; it is not a substitute audit of every possible constructor.

## Repros and validation

No callable repro was fabricated and no Vitest project was run: the selected suite remains pending. Node-only inventory would require `node-24` if a real implementation were identified; Chromium is not applicable. Running an empty TODO would not validate anything.

Executed from the repository root (with `PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`):

```sh
mise exec -- node -p 'JSON.stringify({node:process.version,v8:process.versions.v8})'
mise exec -- node --input-type=module -e 'import assert from "node:assert"; console.log(JSON.stringify({node:process.version,v8:process.versions.v8,hasOwnAssertPredicate:Object.hasOwn(assert,"AssertPredicate"),AssertPredicateType:typeof assert.AssertPredicate,AssertType:typeof assert.Assert}));'
git -C "$HOME/code/clones/node" rev-parse 'v24.19.0^{commit}'
sha256sum node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/assert.d.ts
```

All exited 0. Actual runtime: **Node v24.19.0**, **V8 13.6.233.17-node.51**. The export diagnostic returned `hasOwnAssertPredicate:false`, `AssertPredicateType:"undefined"`, `AssertType:"function"`, corroborating (not replacing) the declaration/source proof. An initial unwrapped `node --version` reported v22.23.1; it was not used for the Node 24 diagnostic. No mocks, fixtures, or state modifications were needed.

## Open questions / coordinator follow-up

- Keep this exact TODO pending as a declaration-only constructor entry. A future inventory/policy decision may distinguish caller-supplied type signatures from runtime built-ins; no generator or manifest changes were made here.
- No runner/configuration change can supply a unique implementation for this alias. A specific supplied constructor would be a different audit target.
- Node annotation delivery has no durable mechanism in this workflow. Any eventual parametric constructor-effect support requires explicit coordinator follow-up, not edits to installed declarations.
