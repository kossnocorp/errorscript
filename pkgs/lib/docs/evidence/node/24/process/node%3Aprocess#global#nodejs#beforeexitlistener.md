# node:process > global > NodeJS > BeforeExitListener

- ID: `node/24/process.test.ts::["node:process","global","NodeJS","BeforeExitListener"]`
- Test: `pkgs/lib/tests/node/24/process.test.ts:7–11`; literal TODO retained.
- Declaration: `pkgs/lib/node_modules/@types/node/process.d.ts:324`, `@types/node@24.19.2`: `type BeforeExitListener = (code: number) => void;`. This is the sole declaration/signature, not an overload set.
- Runtime mapping: none unique. This type describes a supplied callback; it is not an exported function, constructor, accessor, or callable property of `process`.
- Status: blocked (declaration-only callback; no built-in implementation to audit).
- Contract candidate: parametric supplied-code effect, conceptually `(code: number) => Returns<void, E>` for the selected callback's effect `E`; absent that information use `Returns<void, unknown>`, not `never` or a finite built-in-error list. This is not a proposed intrinsic Node implementation annotation.

## Conclusion

No intrinsic exception set can be assigned to this alias. A function satisfying the type may synchronously throw any JavaScript value. Its argument checks, receiver behavior, conversions, invoked getters/proxies, and other operations depend on its body. The `void` return annotation imposes no non-throwing guarantee and supplies no implementation to test.

Consumer channels must remain separate:

- Listener registration (`on`, `once`, etc.) is not invocation of `BeforeExitListener`. Registration validation errors belong to those methods, not this alias.
- Explicit `process.emit('beforeExit', code)` invokes registered callbacks synchronously. An exception from a callback propagates through that invocation; it is not an intrinsic exception manufactured by `BeforeExitListener`.
- Natural `beforeExit` delivery occurs after the event loop runs out of work, not synchronously from an earlier registration call. A thrown callback value is a failure during event delivery and can reach uncaught-exception handling. The event's numeric argument is an exit code, not an error-first callback value.
- An async supplied callback can return a rejecting promise despite the `void` callback type. This is a callback-result/rejection effect, not a promise returned by registration or `emit`. Normal process EventEmitter initialization has captureRejections disabled; opt-in capture routes rejected results through the consumer's rejection/error-event machinery. Neither branch determines this alias's error set.
- There is no prescribed iterator or iteration-time channel. Any deferred work or iterator returned by supplied code requires that code's own effect analysis.
- `beforeExit` is distinct from `exit`. Explicit `process.exit()` and uncaught-exception termination do not imply delivery of `beforeExit`; errors from exit-related APIs must not be attributed to this callback alias.

These observations identify the callback boundary, not a complete audit or annotation of the separate consumer APIs. Host assumptions: ordinary unmodified Node process/EventEmitter behavior; no synthetic callback is treated as a built-in.

## Proof

### Declaration identity and all references

Version-pinned declaration: [@types/node@24.19.2/process.d.ts](https://unpkg.com/@types/node@24.19.2/process.d.ts). The installed file SHA-256 is `614d31f77751e038f30e01641443df8591e12b337c675a44f70675fd5dbf5ae9`; package.json reports `24.19.2`.

- Lines 1, 121–123 place the declaration in `declare module "process"`, `global`, `namespace NodeJS`.
- Line 324 reads verbatim: `type BeforeExitListener = (code: number) => void;`.
- Lines 2096–2100 export `process` and re-export it through `declare module "node:process"`; the managed wrapper does not turn the global type alias into a runtime export.
- Searching the entire installed `@types/node` package for `BeforeExitListener` finds exactly this definition plus six references: `addListener` (1995), `on` (2026), `once` (2040), `prependListener` (2054), `prependOnceListener` (2067), and `listeners` (2080). The first five accept it as a listener; the last returns `BeforeExitListener[]`. These are consumers, not overloads of the selected callable. The related `emit(event: "beforeExit", code: number): boolean` at line 2008 likewise is a different callable.

### Matching Node implementation: consumer, not alias implementation

All following source links are pinned to Node revision `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`. Its [src/node_version.h, lines 25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines version `24.19.0`, matching the explicitly selected runtime. The local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; research used `git show` of the matching revision, not HEAD.

1. [lib/internal/bootstrap/node.js:417–421](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/bootstrap/node.js#L417-L421): `ObjectSetPrototypeOf(origProcProto, EventEmitter.prototype);` and `FunctionPrototypeCall(EventEmitter, process);`. The process object consumes supplied listeners through EventEmitter.
2. [src/api/embed_helpers.cc:42–63](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/api/embed_helpers.cc#L42-L63): after `uv_run` and draining tasks, the loop checks `uv_loop_alive`, then calls `EmitProcessBeforeExit(env)`. This establishes natural event-delivery timing; work scheduled by listeners can make the loop run again.
3. [src/api/hooks.cc:33–50](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/api/hooks.cc#L33-L50): constructs the integer exit code and calls `ProcessEmit(env, "beforeExit", exit_code)`. [src/node_process_events.cc:48–61](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_process_events.cc#L48-L61) builds `{event_string, message}` and calls `MakeCallback` using `env->emit_string()`. No prescribed listener function body is supplied by this path.
4. [lib/events.js:503–537](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L503-L537): single listener `const result = handler.apply(this, args);`; multiple listeners `const result = ReflectApply(handler[i], this, args);`. The multiple-listener `finally` only decrements `handler[kEmitting]`; it does not swallow a callback exception. [Lines 625–631](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L625-L631) likewise have the once wrapper call the supplied `this.listener`. This supports arbitrary supplied-code propagation, not a fixed constructor list.
5. [lib/events.js:248–254](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L248-L254): `// The default for captureRejections is false`. [Lines 350–395](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L350-L395) copy the setting, early-return from `addCatch` when disabled, and otherwise inspect `promise.then`, route rejection via `process.nextTick(emitUnhandledRejectionOrErr, ...)`, and deliver to the rejection hook or `'error'`. These are consumer-side promise/getter/error-event effects, not evidence of an intrinsic `BeforeExitListener` error.
6. [doc/api/process.md:32–45](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/doc/api/process.md#L32-L45), corroborating the implementation distinction: “The listener callback function is invoked with the value of [`process.exitCode`][] passed as the only argument” and “The `'beforeExit'` event is _not_ emitted for conditions causing explicit termination, such as calling [`process.exit()`][] or uncaught exceptions.” The separate native exit emitter is [src/api/hooks.cc:61–92](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/api/hooks.cc#L61-L92), which emits `"exit"`, not `"beforeExit"`.

## Repros and validation

No runtime repro added or executed: there is no unique target implementation. Running the empty TODO or a fabricated throwing callback would not validate a built-in contract. The selected suite retains its original literal `it("TODO", ...)`; zero repros ran. Required project for any future genuine Node target is `node-24`; Chromium is not applicable to this Node-only declaration.

Executed inspection/version commands (repository root):

```sh
git rev-parse HEAD
git -C "$HOME/code/clones/node" rev-parse HEAD
git -C "$HOME/code/clones/node" show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:src/node_version.h
sha256sum pkgs/lib/node_modules/@types/node/process.d.ts
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" node -p 'JSON.stringify(process.versions)'
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec node@24.19.0 -- node -p 'JSON.stringify(process.versions)'
```

Repository HEAD: `9f174c28345f47275237cb814abb6eaeac41588f`. The unqualified PATH command reported Node `22.23.1`, so it was not used as Node 24 evidence. Explicit `mise exec node@24.19.0` succeeded and reported Node `24.19.0`, V8 `13.6.233.17-node.51`. Targeted `git show …:<path> | nl -ba | sed -n '<range>p'` and `git grep` inspected the pinned source ranges above; declaration searches/readbacks established the sole signature and its references. No resources or process listeners were modified.

## Open questions

- The coordinator must decide how declaration-only callback entries should be represented or excluded in a future inventory policy. Until then this entry remains pending, rather than claiming `never throws` or transferring consumer errors onto the alias.
- A callback-specific contract requires the actual supplied implementation/effect parameter; no runtime configuration change can provide one for this type alias.
- Node annotation delivery remains deferred; no declaration annotation, shared runner/configuration, dependency, helper, or inventory change is requested or made.
