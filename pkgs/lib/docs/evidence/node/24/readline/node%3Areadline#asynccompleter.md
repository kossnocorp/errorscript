# node:readline > AsyncCompleter

- ID: `node/24/readline.test.ts::["node:readline","AsyncCompleter"]`
- Test: `pkgs/lib/tests/node/24/readline.test.ts:4-9`; literal TODO retained unchanged.
- Declaration: `node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/readline.d.ts:322-325`, package version `24.19.2`. File SHA-256: `42a350ce4f6f291198dc89833afcc4fd078248b4e1fdc9d05c846c66a670f710`.
- Runtime mapping: supplied callback type, not a runtime export or a uniquely identified Node implementation.
- Status: blocked (declaration-only callback; definitive mapping blocker).
- Contract candidate: implementation-parametric `Returns<void, unknown>` for supplied synchronous effects, not a finite built-in error union. No library annotation is justified by this alias alone. The error-first callback parameter is a separate delivery channel.

## Conclusion

`AsyncCompleter` has one function-type signature, `(line: string, callback: (err?: null | Error, result?: CompleterResult) => void) => void`. It describes code supplied to readline, not an asynchronous built-in named `AsyncCompleter`. The `node:readline` declaration re-exports the `readline` type. Neither its name nor its `void` return guarantees no synchronous throws or establishes a promise-rejection contract.

No intrinsic error constructor can be assigned to this alias: that is an absence of an identified implementation, **not** proof of an empty error set. A supplied implementation may throw any JavaScript value synchronously, invoke the supplied callback (which may itself throw), or arrange later work with its own effects. In the legacy readline consumer, a synchronous throw from the completer is not caught by `_tabComplete`. Passing `callback(error)` instead follows an error-first result-delivery path that resumes input and writes a diagnostic; it does not by itself throw that error or reject a promise. Exceptions while handling results/output are consumer behavior, not an intrinsic contract of this type alias.

Scope: Node `v24.19.0`, `@types/node@24.19.2`, ordinary unmodified Node modules. No tests of a hand-written implementation, call of a nonexistent export, or consumer validation failure are substituted for this inventory entry.

## Proof

### Declaration identity and all signatures

Versioned primary declaration: [@types/node@24.19.2/readline.d.ts](https://unpkg.com/@types/node@24.19.2/readline.d.ts).

- Lines 322-325: `export type AsyncCompleter = (` followed by `line: string,`, `callback: (err?: null | Error, result?: CompleterResult) => void,` and `) => void;`. This is the only defining signature; it has no overloads. Line 326 defines `CompleterResult = [string[], string]`.
- Lines 598-600: `declare module "node:readline" {` / `export * from "readline";`. This is a type re-export, not a second implementation.
- Uses: `Interface`'s protected constructor at lines 105-110, `ReadLineOptions.completer` at line 339, and positional `createInterface` at lines 431-436 accept `Completer | AsyncCompleter`; the options overload at line 437 accepts `ReadLineOptions`. These identify the consumer, not an implementation of the alias.
- A package-wide search also finds the import in `repl.d.ts:12`, `ReplOptions.completer` at line 83 and `REPLServer.completer` at line 235 ([versioned declaration](https://unpkg.com/@types/node@24.19.2/repl.d.ts)). These are uses of the same structural type, not additional overloads or grounds to substitute a REPL completer for every supplied callback.

### Matching Node source and consumer distinction

Research clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; all source below was read with `git show` at **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, not HEAD. [src/node_version.h:25-27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines major/minor/patch as `24`, `19`, `0`, matching the explicit runtime binary.

1. [lib/readline.js:514-523](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/readline.js#L514-L523) exports `Interface`, cursor/line helpers, `createInterface`, `emitKeypressEvents`, and `promises`; there is no `AsyncCompleter` export.
2. [lib/readline.js:101-116](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/readline.js#L101-L116) adapts supplied functions whose length is not 2: `input.completer = (v, cb) => cb(null, completer(v));` and the corresponding positional wrapper. The wrapped target remains supplied code. `createInterface` delegates to `new Interface(...)` at lines 211-212.
3. [lib/internal/readline/interface.js:176-180](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L176-L180) reads `completer = input.completer;`; [lines 220-222](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L220-L222) validate that it is a function, and [line 252](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L252) stores `this.completer = completer;`. Constructor errors here belong to the constructor, not the alias.
4. [lib/readline.js:435-449](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/readline.js#L435-L449): the comment says the legacy completer “takes a callback instead of being an async function.” The code calls `this.completer(string, (err, value) => {` without a surrounding catch. Its callback calls `this.resume();`, then for truthy `err` writes `` `Tab completion error: ${inspect(err)}` `` and returns; otherwise it calls `this[kTabCompleter](lastKeypressWasTab, value)`. This proves the distinction between supplied synchronous throws and error-first delivery, without claiming the consumer cannot independently fail. [Lines 260-264](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/readline.js#L260-L264) redirect the internal tab-completion symbol to `this._tabComplete`.
5. Do not confuse this alias with the base async method: [lib/internal/readline/interface.js:701-714](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L701-L714) uses `value = await this.completer(string);` with catch/finally. It neither defines nor exports `AsyncCompleter`; the legacy override above is explicitly callback-based.

## Repros and validation

No behavioral repros or Vitest projects executed: there is no uniquely identified built-in callable to test. Expected/actual new repro count: **0**. Running the TODO as a passing test would not validate a contract. Chromium is inapplicable to this Node-only entry.

Commands executed from the repository root:

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"; node --version
git -C "$HOME/code/clones/node" rev-parse HEAD
git -C "$HOME/code/clones/node" show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:src/node_version.h
# Targeted, line-numbered git show ranges of lib/readline.js,
# lib/internal/readline/interface.js, and lib/readline/promises.js.
rg -n 'AsyncCompleter' node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node --glob '*.d.ts'
sha256sum node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/readline.d.ts
"$HOME/.local/share/mise/installs/node/24.19.0/bin/node" -p 'JSON.stringify({node:process.version,v8:process.versions.v8,AsyncCompleter:Object.hasOwn(require("node:readline"),"AsyncCompleter"),exports:Object.keys(require("node:readline"))})'
```

The initial PATH-only version check returned `v22.23.1`; it was not used as the target runtime. The explicit binary returned Node `v24.19.0`, V8 `13.6.233.17-node.51`, `AsyncCompleter: false`, and exactly the source export names listed above. This is a mapping check, not an error-contract repro. No resources/state were changed by the checks.

## Open questions

- Coordinator must decide how declaration-only callback aliases receive implementation-dependent contracts or remain pending. No runner/configuration change can supply a unique built-in implementation for this alias.
- Keep this callable's literal TODO. No shared files, manifests, declaration annotation tables, or test suites were changed. Node annotation delivery remains deferred.
