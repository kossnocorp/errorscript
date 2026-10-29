# node:module > Module > InitializeHook

- ID: `node/24/module.test.ts::["node:module","Module","InitializeHook"]`
- Test: `pkgs/lib/tests/node/24/module.test.ts:4-10`; literal TODO retained.
- Declaration: `@types/node@24.19.2`, `module.d.ts:385`: `type InitializeHook<Data = any> = (data: Data) => void | Promise<void>;`. This is the sole occurrence/signature in that package, not an overload family.
- Runtime mapping: declaration-only type of an application-supplied module's named `initialize` export, invoked by the asynchronous loader-hook worker. There is no built-in `Module.InitializeHook` implementation.
- Status: **blocked** (no unique implementation to audit).
- Contract candidate: implementation-parametric `Returns<void | Promise<void>, unknown>`, covering arbitrary synchronous thrown values and rejected promises from supplied code. This is not an intrinsic built-in annotation or a claim that every implementation throws. No finite intrinsic error set or `never` contract is justified for this alias.

## Conclusion

The managed path is a function **type alias** within the merged `Module` namespace, not a constructor, accessor, callable property, or exported runtime function. Its generic `Data` parameter imposes no runtime validation. An implementation may throw synchronously, return a rejected promise, invoke getters/proxies/callbacks with arbitrary effects, or do none of these. Those behaviors depend on the supplied implementation.

In its intended Node loader context, Node invokes the user's `initialize(data)` on the dedicated loader thread (a separate realm), and awaits its result. A synchronous throw or returned rejection becomes a failure of worker-side asynchronous registration. Ordinary `module.register()` blocks for the worker response and can synchronously throw the transported failure on the registering thread. Objects are serialized/deserialized, so object identity across the thread boundary is not promised. This is not a promise returned by `InitializeHook` as a built-in, nor an error-first callback or iterator protocol. Independently scheduled callback/event failures are separate from the hook's completion and depend on application code.

Loader import/parse errors, invalid non-callable hook exports, registration argument/transfer validation, worker serialization errors, and never-settling-loader errors belong to registration/loading infrastructure, not intrinsic errors of this callable type. In particular, a syntax error while importing a hook module is not proof that a successfully invoked `InitializeHook` creates `SyntaxError`. No arbitrary callback repros are offered as built-in proof.

Scope: Node v24.19.0 and the specified declarations; unmodified Node internals. The source establishes the invocation and delivery context, not an exhaustive audit of the separate `register` callable.

## Proof

### Version and declaration identity

- Installed declaration package: `node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/`; `package.json:3` states `"version": "24.19.2"`. Versioned primary declaration: [module.d.ts](https://unpkg.com/@types/node@24.19.2/module.d.ts), lines 4-13, 385, 839-843. Quotes: `namespace Module {`, `type InitializeHook<Data = any> = (data: Data) => void | Promise<void>;`, and `declare module "node:module" { import module = require("module"); export = module; }` (last quote whitespace compacted). Thus the `node:` spelling aliases `module`, whose exported class/namespace is `Module`; the callable entry is only a type. A package-wide `InitializeHook` search found exactly one declaration and no overloads.
- SHA-256 of installed `module.d.ts`: `2d25328278ab945e6ad1e73d831a9e79ba274cb994ebc0470f29c08d2550bcd2`.
- Node source revision used throughout: `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`. [src/node_version.h:25-27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines `NODE_MAJOR_VERSION 24`, `NODE_MINOR_VERSION 19`, `NODE_PATCH_VERSION 0`, matching the explicit runtime probe. Local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; all source excerpts were read with `git show`/`git grep` at the supplied revision, not HEAD.

### Actual exported API and callback selection

- [lib/module.js:8-38](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/module.js#L8-L38): `const { Module } = require('internal/modules/cjs/loader');`, `Module.register = register;`, `module.exports = Module;`. This exports the actual module API; the type alias is not an assigned implementation. The matching runtime reports `typeof require("node:module").InitializeHook === "undefined"` (corroboration, not the sole proof).
- [lib/internal/modules/esm/hooks.js:170-212](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/hooks.js#L170-L212): documented as `Import and register custom/user-defined module loader hook(s).`; `register` imports the supplied module and does `await this.addCustomLoader(urlOrSpecifier, keyedExports, data);`. `addCustomLoader` extracts hooks and ends with `return initialize?.(data);`. Node supplies the argument, not the function body. There is no default initialize implementation in this path.
- [hooks.js:694-719](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/hooks.js#L694-L719): `if (initialize) { acceptedHooks.initialize = initialize; }`. A missing/falsy export is omitted; a truthy non-callable export would fail at the infrastructure's optional call, not constitute a valid callback implementation's intrinsic error contract.

### Worker context and error channel

- [hooks.js:499-540](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/hooks.js#L499-L540): `Abstraction over a worker thread that runs the asynchronous module loader hooks.`; constructed with `new InternalWorker('internal/modules/esm/worker', ...)`.
- [worker.js:226-267](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/worker.js#L226-L267): `response = await ReflectApply(asyncLoaderHooks[method], asyncLoaderHooks, args);`, followed by `catch (exception) { hasError = true; response = exception; }`, then `wrapMessage(hasError ? 'error' : 'success', response)`. Both immediate throws and awaited rejection flow into the worker response. This code does not narrow the failure to built-in Error instances.
- [worker.js:109-132](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/worker.js#L109-L132): simple primitive bodies pass through; other failures use `serialized = serializeError(body);`, recording `serializationFailed` if necessary. Transport is not reference-identity propagation across realms.
- [hooks.js:632-675](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/hooks.js#L632-L675) and [843-845](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/modules/esm/hooks.js#L843-L845): registration calls `makeSyncRequest('register', ...)`, waits using `AtomicsWait`, then unwraps the response. Error handling includes `throw body;` for null/primitives and `throw deserializeError(body.serialized);` otherwise. Infrastructure also handles `ERR_ASYNC_LOADER_REQUEST_NEVER_SETTLED`, `ERR_WORKER_UNSERIALIZABLE_ERROR`, and worker exit. These are not intrinsic error types attributed to this alias.
- Primary Node documentation at the same revision, [doc/api/module.md:1297-1343](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/doc/api/module.md#L1297-L1343), confirms the source mapping: `The exported functions must have specific names and signatures, and they must be exported as named exports.`; `That means it is a different [realm][].`; `The initialize hook is only accepted by register` (inline-code formatting omitted). The promise is `awaited before the main application thread execution resumes.` `registerHooks()` does not accept this hook; synchronous hooks do their own initialization before registration.

## Repros and validation

No Vitest repro is justified: there is no built-in function to invoke under this suite. The existing literal `it("TODO", ...)` is unchanged, and no `never throws` marker or fabricated user-function test was added. Executed test counts: **0**; no Node or Chromium project run is claimed. Node-only inventory would require `node-24` if a real built-in repro became applicable; Chromium is not applicable.

Commands executed (repository root unless otherwise stated):

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
node -p 'JSON.stringify({node:process.version,v8:process.versions.v8})'
# v22.23.1 / 12.4.254.21-node.56: shell PATH alone did not select Node 24.

~/.local/share/mise/installs/node/24.19.0/bin/node -p \
  'JSON.stringify({node:process.version,v8:process.versions.v8,InitializeHook:typeof require("node:module").InitializeHook})'
# {"node":"v24.19.0","v8":"13.6.233.17-node.51","InitializeHook":"undefined"}

git -C ~/code/clones/node rev-parse HEAD
git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:src/node_version.h
# Targeted git show/git grep reads of the source ranges cited above, at that revision.
sha256sum node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/module.d.ts
```

The runtime probe only inspects exports/version; it does not register hooks or create persistent worker/resource state. No teardown, shared configuration, dependency, generated inventory, or annotation-table edits were needed.

## Open questions

- To replace the TODO with behavioral evidence, the inventory/coordinator must identify a concrete implementation, or provide a policy for declaration-only callback effects. A callback type alone cannot supply a built-in error set. Keep this entry pending rather than reclassifying it as `never`.
- External Node declaration annotation delivery remains deferred; no annotation has been applied. No runner/configuration change is requested for this blocker. Any future Node test must explicitly select the installed v24.19.0 binary rather than rely on the observed default PATH.
