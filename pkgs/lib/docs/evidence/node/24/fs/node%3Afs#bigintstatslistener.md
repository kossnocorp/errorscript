# node:fs > BigIntStatsListener

- ID: `node/24/fs.test.ts::["node:fs","BigIntStatsListener"]`
- Test: `pkgs/lib/tests/node/24/fs.test.ts:4–9`; literal TODO at line 6 retained unchanged.
- Declaration: `node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/fs.d.ts:3663`, one call signature: `export type BigIntStatsListener = (curr: BigIntStats, prev: BigIntStats) => void;`. File SHA-256: `e6a2a1eecdb48e9a9eb36d115e212664f20710530e5ec32bbea84bf36979d2c7`.
- Runtime mapping: declaration-only, application-supplied callback type; no unique Node implementation or runtime export.
- Status: blocked
- Contract candidate: unresolved per supplied implementation; conservative callback-shape candidate `(curr: BigIntStats, prev: BigIntStats) => Returns<void, unknown>`, not an established built-in contract. No `Throws<F>` candidate: the alias supplies no concrete callable `F`. External Node annotation delivery is deferred.

## Conclusion

This inventory entry is a type alias, not a Node function. Neither a finite intrinsic error list nor `never` can be assigned from this declaration. A supplied function can throw any JavaScript value synchronously when invoked. When used by `watchFile`, invocation occurs during later `change` event delivery, not during the initiating registration call. Its effects belong to the supplied implementation; Node's registration/path validation errors are not errors created by `BigIntStatsListener`.

The `void` callback return type does not establish that every supplied implementation returns normally, nor that supplied async implementations cannot reject. Promise effects, proxy/accessor behavior, receiver requirements, or deferred iteration effects depend on the actual supplied implementation. There is no implementation here whose reachable validation/algorithm paths can be exhaustively audited. No fixture-defined callback repro would prove a built-in contract.

## Proof

### Declaration identity and all uses

Version-pinned declaration source: [@types/node 24.19.2 fs.d.ts](https://unpkg.com/@types/node@24.19.2/fs.d.ts).

- Line 3663 says exactly `export type BigIntStatsListener = (curr: BigIntStats, prev: BigIntStats) => void;`. It is one callable type signature, with no constructor, accessor, implementation, or overload of its own.
- Lines 4775–4777 say `declare module "node:fs" { export * from "fs"; }`, resolving the managed namespace to that alias.
- Lines 3619–3627 place `listener: BigIntStatsListener` in the `watchFile` overload with `bigint: true` options (or `undefined`). Adjacent overloads at 3610–3618 and 3632 use `StatsListener`, a distinct alias. Line 3646 accepts `listener?: BigIntStatsListener` in `unwatchFile`; line 3645 uses `StatsListener`. Searching the entire installed declaration package found only these three occurrences of `BigIntStatsListener`: its definition and these two parameter uses. These consumers are separate callables, not implementations of the alias.

### Matching Node implementation

All following citations use Node revision `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`. Its [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines major `24`, minor `19`, patch `0`, matching the explicitly selected runtime. The clone's current HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; it was not used as the audited implementation.

1. [lib/fs.js:2653–2686](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs.js#L2653-L2686): `validateFunction(listener, 'listener');`, then `stat = new watchers.StatWatcher(options.bigint);` if needed, and `stat.addListener('change', listener);`. Node accepts and stores the caller's function; it does not create a `BigIntStatsListener` implementation or invoke this listener as the registration operation's callback. Validation belongs to `watchFile`.
2. [lib/internal/fs/watchers.js:130–169](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/watchers.js#L130-L169): `ObjectSetPrototypeOf(StatWatcher.prototype, EventEmitter.prototype);` and the native handle's `this._handle.onchange = onchange;`. The `onchange` handler can suppress an unchanged initial failure; otherwise it executes `self.emit('change', getStatsFromBinding(stats), getStatsFromBinding(stats, kFsStatsFieldsNumber));`. This identifies the later event-delivery consumer, not an implementation of the supplied listener.
3. [lib/events.js:503–535](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L503-L535): a single listener executes via `const result = handler.apply(this, args);`; multiple listeners execute via `const result = ReflectApply(handler[i], this, args);`. The latter has a `finally` decrement, not a catch. Synchronous listener throws therefore escape event dispatch without conversion to a fixed error class. Returned values may pass to `addCatch`; [lines 356–375](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/events.js#L356-L375) start `if (!that[kCapture]) { return; }`, and otherwise inspect `promise.then` and arrange rejection/error-event delivery. This is consumer/configuration-dependent promise handling, not a guaranteed rejection channel of the callback alias or a rejection from `watchFile`.
4. [lib/fs.js:2695–2715](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs.js#L2695-L2715): `stat.removeListener('change', listener);` confirms the other alias use identifies the supplied callback for removal rather than invoking it.

The public export object at [lib/fs.js:3300 onward](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs.js#L3300) exports `watchFile`, not a callback implementation named `BigIntStatsListener`. The runtime check below corroborates, but is not the basis for an empty-error claim.

## Repros and validation

No Vitest repro was added or executed: the required Node project would be `node-24`, but there is no unique callable under test. Running the unchanged TODO or invoking a fabricated callback would not validate this entry. Chromium is not applicable to this Node declaration. No resources, watcher state, mocks, or polyfills were created.

Executed inspection commands from the repository root:

```sh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
node --version
git -C "$HOME/code/clones/node" rev-parse HEAD
git -C "$HOME/code/clones/node" show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:src/node_version.h | head -40
sha256sum node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/fs.d.ts
"$HOME/.local/share/mise/installs/node/24.19.0/bin/node" -e 'const fs = require("node:fs"); console.log(JSON.stringify({node:process.version,v8:process.versions.v8,BigIntStatsListener:Object.hasOwn(fs,"BigIntStatsListener"),watchFile:typeof fs.watchFile}));'
```

The initial PATH-selected Node was **v22.23.1**, not the requested runtime; it was not used for behavioral evidence. The explicit Node 24 command succeeded with:

```json
{"node":"v24.19.0","v8":"13.6.233.17-node.51","BigIntStatsListener":false,"watchFile":"function"}
```

Pinned source ranges were inspected with `git show <revision>:<path> | nl -ba | sed -n '<ranges>p'`: `lib/fs.js` 2640–2689, 2690–2718, 3295–3310, 3373–3400; `lib/internal/fs/watchers.js` 130–176; `lib/events.js` 350–375, 495–540. Installed declarations were searched/read at the lines cited above.

## Open questions

Definitive blocker: the queue selected a declaration-only supplied callback, not a unique runtime implementation. Retain TODO pending coordinator treatment of callback-type inventory entries or a separately specified concrete implementation. No runner/configuration/dependency change can supply the missing implementation; none requested. No annotation tables or other callable suites were edited.
