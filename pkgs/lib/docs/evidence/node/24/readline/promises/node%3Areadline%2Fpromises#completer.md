# node:readline/promises > Completer

- ID: `node/24/readline/promises.test.ts::["node:readline/promises","Completer"]`
- Test: `pkgs/lib/tests/node/24/readline/promises.test.ts`, exact managed suite above.
- Declaration: `@types/node@24.19.2`, `readline/promises.d.ts:118`: `type Completer = (line: string) => CompleterResult | Promise<CompleterResult>`; one call signature, no overloads. Lines 159–161 re-export it through `node:readline/promises`. `readline.d.ts:326` defines `CompleterResult = [string[], string]`.
- Runtime mapping: none for the type alias itself; a caller-supplied function consumed by the promises readline Interface's tab-completion implementation.
- Status: blocked (no unique built-in implementation).
- Contract candidate: implementation-parametric `Returns<CompleterResult | Promise<CompleterResult>, E_sync | E_reject>`; both effects are unknown for an arbitrary supplied Completer and can contain any JavaScript value. `Throws<F>` requires a callable, not an error type parameter. No justified finite intrinsic error list or `never` contract for this inventory entry.

## Conclusion

`Completer` is a declaration-only callback type, not a Node-exported callable. A concrete supplied implementation may throw synchronously or return a promise that rejects with arbitrary values. Neither behavior establishes an intrinsic Node error constructor for this type. This is not the separate callback-style `readline.AsyncCompleter` (which accepts an error-first callback).

The Node consumer catches both synchronous invocation errors and awaited rejections, writes a diagnostic, and resumes input. Thus these supplied failures are not automatically synchronous throws from `createInterface`, nor error-first callback/event delivery. Consumer setup validation, result processing, output, and resume failures belong to the consumer, not to the callback alias. No claim that those consumer operations are error-free is made. Keep the literal TODO; an invented callback repro would only test invented code.

## Proof

### Pinned declaration

Primary package source: [readline/promises.d.ts, @types/node 24.19.2](https://unpkg.com/@types/node@24.19.2/readline/promises.d.ts), lines 118–124 and 151–161; [readline.d.ts](https://unpkg.com/@types/node@24.19.2/readline.d.ts), lines 321–326.

> `type Completer = (line: string) => CompleterResult | Promise<CompleterResult>;`

> `completer?: Completer | undefined;`

Both `createInterface` overloads accept supplied implementations: the positional overload at 151–156 takes `completer?: Completer`; the options overload at 157 takes `ReadLineOptions`, whose property is declared at 123. These consumers do not supply a unique implementation of the alias. There are no additional alias call overloads. The non-promises `readline.Completer` and `AsyncCompleter` are distinct declarations.

Local installed declaration SHA-256:

- `readline/promises.d.ts`: `77fbe5eecb6fac4b6242bbf6eebfc43e98ce5ccba8fa44e0ef6a95c945ff4d98`
- `readline.d.ts`: `42a350ce4f6f291198dc89833afcc4fd078248b4e1fdc9d05c846c66a670f710`

### Pinned Node implementation and consumer handling

All Node citations use revision `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae` (release commit `2026-08-03, Version 24.19.0 'Krypton' (LTS)`). [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) identifies 24.19.0. The local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; research used `git show` of the matching release, not that HEAD.

1. [lib/readline/promises.js:12–17,29,61–69](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/readline/promises.js#L61-L69): `createInterface` does `return new Interface(input, output, completer, terminal);`; the module exports only `Interface`, `Readline`, and `createInterface`. `Interface` extends the internal Interface. No runtime `Completer` export exists.
2. [lib/internal/readline/interface.js:160–179,220–222,252,361](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L160-L179): the internal Interface extends `InterfaceConstructor`. Options supply `completer = input.completer;`, the constructor validates that it is a function if supplied, then assigns `this.completer = completer;`. This retains caller code rather than installing a built-in implementation. The constructor's `throw new ERR_INVALID_ARG_VALUE('completer', completer);` is not a throw by `Completer`.
3. [lib/internal/readline/interface.js:701–714](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L701-L714):

   ```js
   try {
     value = await this.completer(string);
   } catch (err) {
     this[kWriteToOutput](`Tab completion error: ${inspect(err)}`);
     return;
   } finally {
     this.resume();
   }
   this[kTabCompleter](lastKeypressWasTab, value);
   ```

   The same catch handles a synchronous callback throw and a rejection of its result (including thenable assimilation failures for out-of-type values). The diagnostic path does not rethrow `err`. Output/resume can have separate effects. Successful-result destructuring/processing at line 716 onward is outside this catch and is consumer behavior, not callback behavior.
4. [lib/internal/readline/interface.js:1518–1527](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/readline/interface.js#L1518-L1527): the Tab key handler checks `typeof this.completer === 'function'` and invokes `this[kTabComplete](lastKeypressWasTab);` without awaiting its promise. Consequently a separate consumer failure must not be conflated with the supplied callback's already-caught error or an error thrown during interface creation.

## Repros and validation

No behavioral repros or Vitest projects executed: no unique callable exists to exercise. The TODO is deliberately retained; running it as a passing empty test would not validate an error contract. Chromium is not applicable to this Node-only declaration. No mocks, resources, or shared configuration changes.

Commands executed from repository root:

```sh
git -C ~/code/clones/node rev-parse HEAD
git -C ~/code/clones/node show -s --format='%H %s' cdc1b38d40cb567b7ad0b39c86addf830a0af0ae
git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/readline/promises.js | nl -ba
git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/internal/readline/interface.js | nl -ba | sed -n '160,185p;215,255p;695,750p;1510,1532p'
git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:src/node_version.h | head -40
sha256sum pkgs/lib/node_modules/@types/node/readline/promises.d.ts pkgs/lib/node_modules/@types/node/readline.d.ts
"$HOME/.local/share/mise/installs/node/24.19.0/bin/node" -e 'console.log(JSON.stringify({node:process.version,v8:process.versions.v8,exports:Object.keys(require("node:readline/promises")),types:require("./pkgs/lib/node_modules/@types/node/package.json").version}))'
```

The runtime mapping probe returned Node `v24.19.0`, V8 `13.6.233.17-node.51`, exports `["Interface","Readline","createInterface"]`, and types `24.19.2`. It is an export/version check, not an error repro. The supplied PATH alone initially selected Node `v22.23.1`; the explicit 24.19.0 binary above avoided using that mismatched runtime.

## Open questions

The coordinator must decide how declaration-only, user-implemented callback effects should be represented in the inventory/annotation pipeline. Until a justified implementation-specific or parametric contract is supported, this entry remains blocked and pending. No runner/configuration change can provide a missing unique implementation. Node annotation delivery remains deferred; nothing was annotated.
