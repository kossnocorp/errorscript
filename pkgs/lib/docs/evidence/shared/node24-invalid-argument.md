# Node 24 invalid-argument diagnostic dependencies

Scope: Node 24.19.0, revision `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`,
unmodified built-ins. This is a shared submechanism proof, not a callable audit.
Consumers must prove that their validator reaches `determineSpecificType`, with
the same inspection options and no intervening catch. `ERR_INVALID_ARG_VALUE`
and other formatters are not automatically equivalent.

## Proof and dependency paths

[`lib/internal/errors.js:995–1043`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L995-L1043):

- Functions: ``return `function ${value.name}`;`` — read `input.name`, then ToString.
- Objects: `if (value.constructor && 'name' in value.constructor)` followed by
  ``return `an instance of ${value.constructor.name}`;`` — repeated constructor
  reads, a `has` operation on the constructor, a name read, then ToString.
- Fallback: `inspect(value, { depth: -1 })` — reachable only when the constructor
  check does not select the name branch.

ToString on an object obtains/calls `Symbol.toPrimitive` or performs ordinary
conversion (`toString`, then `valueOf` for hint string). Abrupt completions
propagate; a Symbol result or failure to produce a primitive can create TypeError.
See ECMA-262 [ToPrimitive](https://tc39.es/ecma262/#sec-toprimitive),
[OrdinaryToPrimitive](https://tc39.es/ecma262/#sec-ordinarytoprimitive), and
[ToString](https://tc39.es/ecma262/#sec-tostring). The caller proofs pin the spec
revision used in their original audit.

[`inspect.js:835–932`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util/inspect.js#L835-L932)
unwraps proxies before `const maybeCustom = value[customInspectSymbol]`, then uses
`FunctionPrototypeCall(maybeCustom, context, ...)`. Thus a getter on the inspected
target and the hook invocation are distinct dependencies; an outer proxy trap on
that symbol is not an equivalent fixture. Neither ordinary hook failure is caught.

## Repro reuse and limits

`tests/support/node-diagnostics.ts` supplies named factories for representative
paths: constructor get, constructor `has name`, name get, coercion-hook get/call,
ordinary conversion get/call, function name get/coercion, and custom-inspection get/call.
Each consumer asserts identity around its own invocation; these factories do not
call the validator or `inspect` themselves. The shared factories are **not an
exhaustive audit** of returned object graphs, coercion fallbacks, or inspection.
Caller suites retain their additional native namespace, recursion, iteration,
transformed-error and negative-control mechanisms.

No finite list of sentinels enumerates arbitrary effects. A single sentinel tests
each dependency; the source proves unchanged propagation. Error classes created
by the diagnostic algorithm are recorded separately in caller evidence.

Declared-input reachability is caller-specific: primitive `path` strings cannot
reach these nonstring diagnostics, but a TypeScript `Uint8Array` proxy still has
that static type while failing the native ArrayBuffer-view brand check. Crypto
and StringDecoder contain cast-free repros for that distinction. Do not infer
typed unreachability merely because a minimal diagnostic fixture uses a cast.
