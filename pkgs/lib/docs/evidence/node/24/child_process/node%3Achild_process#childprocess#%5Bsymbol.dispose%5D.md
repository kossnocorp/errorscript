# node:child_process > ChildProcess > [Symbol.dispose]

- ID: `node/24/child_process.test.ts::["node:child_process","ChildProcess","[Symbol.dispose]"]`
- Test: `pkgs/lib/tests/node/24/child_process.test.ts`, selected managed suite only.
- Declaration: `@types/node@24.19.2/child_process.d.ts:314`, `[Symbol.dispose](): void;` on `ChildProcess extends EventEmitter` (line 85). This is the sole overload. `node:child_process` reexports `child_process` at lines 1478–1480; stream-specialized subclasses inherit the method, not distinct implementations. [Pinned declaration](https://unpkg.com/@types/node@24.19.2/child_process.d.ts).
- Runtime mapping: exported `ChildProcess` is imported from `internal/child_process` in `lib/child_process.js:88–94` and exported at line 1065. Actual callable: `ChildProcess.prototype[SymbolDispose]`, `lib/internal/child_process.js:561–565`.
- Status: **blocked** — seven verified repro/control tests; literal TODO retained for native failure/event-delivery validation.
- Contract candidate (not installed): `Returns<void, TypeError | Error | ReceiverEffects | SynchronousListenerEffects>`. The effect parameters may contain **any thrown JS value**, so a non-parametric fallback is `Returns<void, unknown>`. `SyntaxError` in the tests is supplied-code propagation, not an intrinsic parser error. The finite intrinsic candidates are `TypeError` and OS-originated `Error` (the latter source-proved but not reproduced here).

## Conclusion

The method synchronously reads `this.killed`; if falsy it dynamically calls `this.kill()` with **no arguments**, discards that result, and returns `undefined`. There is no ChildProcess brand check, promise, iterator, or argument conversion. A truthy `killed` value avoids the call entirely; Boolean testing does not call user conversion hooks. Extra supplied arguments cannot select an invalid signal.

Synchronous intrinsic `TypeError` includes nullish receivers, noncallable `kill`, revoked/invariant-violating receiver proxies, and failure of the strict-mode `killed = true` write after a successful native kill. Getters, setters, proxy traps, and a supplied receiver's callable `kill` propagate arbitrary values without wrapping. The borrowed-method tests do not replace the built-in under audit or its native kill implementation.

With the normal native `kill`, `SIGTERM` is always selected. A missing handle and `ESRCH` return normally. Native `EINVAL`/`ENOSYS` directly throw an `ErrnoException` whose public constructor is exactly `Error`; other native errors (notably `EPERM`) synchronously emit an `error` event. With a listener that returns normally, that emission is **not a throw**. Without a listener, EventEmitter rethrows the same native Error synchronously through disposal. Throwing error listeners/error monitors propagate their own arbitrary thrown value synchronously. These failure channels remain unvalidated rather than being hidden by the passing supplied-Error test.

Exit/close events and their listener exceptions occur later, via the child-process lifecycle, not as a rejection of disposal. EventEmitter capture-rejection handling can schedule later rejection/error delivery from listener-returned promises; disposal still returns `undefined`. A supplied `kill` returning a rejected promise is likewise not awaited or returned by disposal.

Scope: Node 24.19.0, ordinary resource availability, no global monkey-patching or fabricated native handles. Linux is the tested OS; Windows and other libuv platform differences below are source-only. Termination/OOM and unsafe native-handle corruption are excluded.

## Proof

### Revisions

Node source used throughout: **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, read with `git show` in `~/code/clones/node`. Its `src/node_version.h:25–27` defines `24`, `19`, `0`; `deps/v8/include/v8-version.h:11–14` defines `13.6.233.17`. Runtime reports Node `24.19.0`, V8 `13.6.233.17-node.51`, libuv `1.52.1`, Linux. The clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, **not** the version used for proof. The direct shell's Node was 22.23.1; only repository `mise` commands were used for validation. A diagnostic `mise exec` from inside the Node clone resolved 26.12.1 and was not used for tests or version matching.

ECMA-262 local HEAD: **`40ce6f206b5669e69bf010a38267ae24a2843404`**; used for the JavaScript operations in this Node implementation, not as evidence of a different engine build.

### Dispatch and native kill

1. [Node `internal/child_process.js:534–565`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/child_process.js#L534-L565): disposal says `if (!this.killed) { this.kill(); }`. Kill selects `convertToValidSignal(sig === undefined ? 'SIGTERM' : sig)`. It checks `this._handle`, calls `this._handle.kill(signal)`, writes `this.killed = true` on zero, silently accepts `UV_ESRCH`, and otherwise either `throw new ErrnoException(err, 'kill')` for `UV_EINVAL`/`UV_ENOSYS` or `this.emit('error', new ErrnoException(err, 'kill'))`. Every return from kill is discarded by disposal. The file begins with `'use strict';`, so failed property writes throw.
2. [Node `internal/util.js:369–379`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util.js#L369-L379): `const signalName = signals[StringPrototypeToUpperCase(signal)]; if (signalName) return signalName;`. The fallback `throw new ERR_UNKNOWN_SIGNAL(signal)` is **not reachable for normal disposal**: signal is the fixed string `SIGTERM`, exported unconditionally by [`src/node_constants.cc:689`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_constants.cc#L689). No supplied argument enters conversion. The constants are from `internalBinding('constants').os` (`internal/util.js:62`).
3. [Node `src/process_wrap.cc:344–359`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/process_wrap.cc#L344-L359): unwrap native receiver, convert the numeric signal with `Int32Value`, then `int err = uv_process_kill(&wrap->process_, signal);`. Normal disposal supplies a valid native handle and an integer constant; this conversion invokes no supplied conversion hook. Windows permits SIGTERM unchanged.
4. [Bundled libuv Unix `process.c:1097–1114`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/unix/process.c#L1097-L1114): `return uv_kill(process->pid, signum);`; `if (kill(pid, signum)) ... return UV__ERR(errno);` otherwise zero. Thus OS permission/security-policy failures can reach the error-event branch; successful same-user child termination does not establish that failure is impossible. The `__MVS__` branch may translate zombie `EPERM` to success after `waitid`. SIGTERM being valid means invalid-signal EINVAL is not an ordinary Linux argument path.
5. [Bundled libuv Windows `process.c:1294–1378`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/uv/src/win/process.c#L1294-L1378): SIGTERM selects `TerminateProcess`; success returns zero; already-exited processes can map to `UV_ESRCH`, other failures use `uv_translate_sys_error(err)`. An invalid process handle returns `UV_EINVAL`. The unsupported-signal `UV_ENOSYS` branch is not selected by SIGTERM. No claim is made that all these branches can be forced on an intact, successfully spawned Linux child.
6. [Node `internal/errors.js:721–746`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L721-L746): `class ErrnoException extends Error`; sets `errno`, `code`, `syscall`; `get ['constructor']() { return Error; }`. This is the concrete public constructor to assert in a future native-failure repro; e.g. permission denial requires `code === 'EPERM'` and `syscall === 'kill'`.

### Events are not automatically initiating-call throws

7. [Node `events.js:456–535`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L456-L535): the error monitor runs first; `// If there is no 'error' event listener then throw.` and `throw er; // Unhandled 'error' event`. A native `ErrnoException` is already an Error, so the non-Error `ERR_UNHANDLED_ERROR` construction is not the ordinary native-error path. Listeners run with `handler.apply(this, args)` or `ReflectApply(handler[i], this, args)` without a swallowing catch. This proves both synchronous unhandled-error throwing and listener-effect propagation, but **not** that a handled native error must throw.
8. [Node `events.js:356–395`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L356-L395): capture-rejection handling reads/calls a listener result's `then`; a synchronous failure causes `that.emit('error', err)` (potentially a synchronous propagated throw), while rejection callbacks schedule `process.nextTick(emitUnhandledRejectionOrErr, ...)`. This later channel belongs to event delivery, not a promise returned by disposal.
9. [Node `internal/child_process.js:256–306`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/child_process.js#L256-L306): initial `killed = false`; `_handle.onexit` closes the handle, sets `_handle = null`, then emits an error for a negative spawn exit code or an `exit` event otherwise. [`maybeClose`, lines 1137–1142](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/child_process.js#L1137-L1142) emits `close` after counting resource closure. Native exit callback is [`process_wrap.cc:362–377`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/process_wrap.cc#L362-L377). These deferred callbacks are not run by the synchronous dispose body. Tests await child closure in cleanup and do not use spawn failures as dispose repros.

### Receiver operations and arbitrary propagation

ECMA links below pin the reviewed spec revision; anchors identify the normative algorithms.

10. [GetValue, `spec.html:4472–4492`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L4472-L4492), `sec-getvalue`: `Let _baseObj_ be ? ToObject(...)` and `Return ? ... [[Get]](...)`. `sec-toobject` rejects undefined/null with TypeError. Thus nullish `this` fails inside disposal's first property read, not argument evaluation. Other primitives are boxed for reads but normally lack callable `kill`.
11. [EvaluateCall, `spec.html:19865–19890`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L19865-L19890), `sec-evaluatecall`: `If IsCallable(_func_) is *false*, throw a *TypeError* exception.` and `Return ? Call(...)`. This proves noncallable kill validation and exact propagation from a supplied receiver's method.
12. [OrdinaryGet, `spec.html:13337–13358`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13337-L13358), `sec-ordinaryget`: `Return ? Call(_getter_, _receiver_).` This applies to `killed`, `kill`, and any traversed accessors/prototype getters on borrowed receivers, including native kill's `_handle` access. No finite Error-class list bounds their effects.
13. [Proxy [[Get]], `spec.html:16292–16318`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16292-L16318): `Perform ? ValidateNonRevokedProxy(_obj_)`, `Let _trapResult_ be ? Call(...)`, and invariant violations `throw a *TypeError* exception`. `sec-validatenonrevokedproxy` throws TypeError when `[[ProxyTarget]]` is null. Repros exercise revoked and lying proxies and identity propagation from a throwing trap.
14. [PutValue, `spec.html:4499–4526`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L4499-L4526), `sec-putvalue`: `Let _succeeded_ be ? ... [[Set]](...)`; `If _succeeded_ is *false* and ... [[Strict]] is *true*, throw a *TypeError* exception.` `sec-ordinarysetwithowndescriptor` returns false for nonwritable properties and invokes supplied setters with `? Call`. Consequently a successful OS kill can still be followed by a synchronous TypeError or supplied setter exception. Repros alter only the configurable public `killed` descriptor and restore it in finally, never mocking the native kill or handle.
15. [ToBoolean, `spec.html:5084–5100`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L5084-L5100), `sec-toboolean`: after primitive false cases, `Return *true*.` No object-to-primitive conversion occurs for `!this.killed`; a truthy object suppresses kill without executing its hooks.

## Repros and validation

All seven tests carry `node-only` runtime tags. Only `node-24` is applicable; Node declaration files are not included in the Chromium project.

| Scenario / test name                                                  | Observation                                                                                               |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| invalid receiver or noncallable kill / `throws TypeError`             | null, undefined, primitive, missing and numeric kill; exact constructor                                   |
| revoked proxy and proxy invariants / `throws TypeError`               | revoked get and incompatible frozen-property trap result; exact constructor                               |
| successful native kill cannot write killed / `throws TypeError`       | real live local child, nonwritable public property, synchronous exact constructor                         |
| receiver getter propagation / `throws Error`                          | **supplied** Error identity and constructor, not native OS Error coverage                                 |
| successful native kill invokes supplied setter / `throws SyntaxError` | real native kill followed by **supplied** SyntaxError identity and constructor                            |
| `throws non-Error value`                                              | receiver proxy get, kill getter, and dynamic method propagate the exact sentinel object                   |
| `returns undefined without coercion and on repeated disposal`         | truthy killed skips coercion/kill lookup; live child's successful/repeated disposal; already-closed child |

Run from the repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/node tests/node/24/child_process.test.ts \
  -t '^node:child_process > ChildProcess > \[Symbol\.dispose\] > (?:.* > )?(?:throws |returns )' \
  --maxWorkers 1 --reporter verbose
```

Actual result: Vitest **5.0.5**, project **node-24**, **1 file passed; 7 tests passed, 23 skipped (30 total)**. The exact escaped managed path plus descendant test-name boundary selected all seven intended tests and no other callable. The retained TODO and all other callable TODOs were skipped, not counted as evidence. Node **24.19.0**, V8 **13.6.233.17-node.51**, libuv **1.52.1**. Tests spawn only `process.execPath` with ignored stdio; finally restores descriptors, force-terminates any remaining live child, and awaits close. No mocks, polyfills, external network, dependency/configuration changes, or generated inventory changes.

The same test command was repeated after formatting with the same 7/23 count. `mise exec -- pnpm exec oxfmt --check pkgs/lib/tests/node/24/child_process.test.ts 'pkgs/lib/docs/evidence/node/24/child_process/node%3Achild_process#childprocess#%5Bsymbol.dispose%5D.md'` passed after correcting the initial formatting findings. Targeted `git diff --check` passed.

## Open questions / precise blocker

The source-established **native Error** path needs a real deterministic OS-denial fixture. This worker runs Linux as uid 1000; its freshly spawned Node children have the same ownership, and ordinary SIGTERM succeeds. Fabricating `_handle.kill` return values would mock the mechanism, and targeting unrelated processes would be unsafe. The passing supplied-Error getter repro is explicitly not a substitute.

A directly constructed, never-spawned `new ChildProcess()` is also not a validated native fixture: its constructor allocates `new Process()` (`internal/child_process.js:268`), but [`ProcessWrap` construction at `src/process_wrap.cc:110–116`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/process_wrap.cc#L110-L116) calls `MarkAsUninitialized()`. Disposal does not check this state before entering native kill. No claim is made about its uninitialized native-process fields or safe signaling behavior; do not try this in the shared test runner. Further investigation requires an isolated process/signal namespace or equivalent containment, not a fabricated handle. This lifecycle boundary remains open alongside OS-denial validation.

Coordinator requirement: approve/provide a test-local isolated native sandbox fixture (for example, a Linux seccomp launcher applying an errno-denial policy for the test subprocess's `kill` syscall, with control/cleanup held by an unsandboxed parent), or a supported isolated credential-changing fixture. Minimal shared change would be a Linux-only support launcher plus an explicit capability tag/dependency check, **not** changes to ChildProcess or its handle. Then exercise separately (a) native EPERM without listener: exact Error constructor/code/syscall, (b) listener consumes the error: returns undefined, (c) error listener/monitor throws supplied sentinel: identity propagation; optionally EINVAL/ENOSYS under an explicit OS policy. Capture-rejection re-emission and later lifecycle delivery should not be conflated with disposal rejections. No such helper/config change was made by this worker. Windows native failure paths remain source-only and should be checked if Windows becomes a required runtime.

Annotation delivery for Node is deferred by coordinator instruction. No installed declaration or annotation table was changed; the candidate above is not an annotation claim.
