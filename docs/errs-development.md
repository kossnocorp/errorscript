# Developing the ErrorScript forks

## Run in VS Code

From the repository root, run `mise :symlink` to build the development compiler and link it as `~/.local/bin/errs`, following Genotype's CLI setup. With `~/.local/bin` on PATH, use `errs --errorscript -p path/to/project --noEmit` for command-line inference. Re-running the task refreshes the link.

Open this repository and launch **errorscript:dev** (F5). The prelaunch task runs the extension's `mise :build:dev`, builds `subs/errs` with its existing Cargo dev profile, bundles the isolated extension module, and opens `examples/errs` in an Extension Development Host. Hover function names and calls in `index.ts`. The example associates its TS/JS files with ErrorScript automatically.

In other projects, use the language picker to select **ErrorScript**, **ErrorScript React**, **ErrorScript JavaScript**, or **ErrorScript JavaScript React**. Files retain their usual extensions; tsrs chooses parsing rules from `.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`, `.mjs`, or `.cjs`. These modes currently support saved file URIs, including unsaved edits to those files.

For persistent associations:

```json
{
  "files.associations": {
    "*.ts": "errorscript",
    "*.tsx": "errorscriptreact",
    "*.mts": "errorscript",
    "*.cts": "errorscript",
    "*.js": "errorscriptjavascript",
    "*.jsx": "errorscriptjavascriptreact",
    "*.mjs": "errorscriptjavascript",
    "*.cjs": "errorscriptjavascript"
  }
}
```

The extension starts the bundled `tsrs --errorscript --lsp -stdio`. The hover contains ordinary TS information followed by ErrorScript's `throws`, `rejects`, or deferred iteration errors. Native LSP features such as completion and navigation also serve ErrorScript documents. Ordinary TypeScript/JavaScript documents continue using their built-in editor providers, while their unsaved text is synchronized to ErrorScript for dependency inference.

Inferred error types are clickable: custom errors open their declarations, and built-in errors open the packaged standard-library declarations. The links use native type/symbol identities, including aliases and union constituents.

To require handling errors from root-level execution, add a sibling of `compilerOptions` in `tsconfig.json`:

```json
{
  "errorOptions": {
    "noUnhandledRootErrors": true
  }
}
```

The option defaults to off, inherits through `extends`, and accepts an explicit `false` override. It reports escaping inferred throws/rejections, including unknown/any effects from unmodelled external calls; pure calls remain exempt. Use `try/catch` for synchronous errors, and `.catch(...)` (or `await` inside `try/catch`) for promises. Handler failures/rethrows still need handling, including unmodelled `console.error` calls; an unawaited rejecting promise inside a `try` is not handled. Errors appear in Problems and CLI type checking (`errs --errorscript -p . --noEmit`). The extension provides JSON completion/validation for `errorOptions`.

Use `Returns<Value, Error>` to check a function's return value and exact inferred error type:

```ts
function parse(): Returns<number, TypeError> {
  throw new TypeError("invalid input");
}
```

Explicit contracts are always checked in ErrorScript mode. Missing or extra error types produce diagnostic 98002; `never` means no inferred errors. To require contracts on throwing user-written functions, set `errorOptions.noImplicitReturns: true` (diagnostic 98003). This is separate from `compilerOptions.noImplicitReturns`. Pure functions need no annotation. Async functions can use `Returns<Promise<Value>, Error>` or `Promise<Returns<Value, Error>>`. Both error options default to off and support inheritance/false overrides. See `subs/errs/docs/ERRORSCRIPT.md` for exact-match rules, uncertainty, and scope.

Diagnostic 98003 shows the concrete inferred `Returns<…>` type. Use its **Add return type Returns<…>** Quick Fix to insert the annotation (or a JSDoc return annotation in JS). The fix uses the current document's native types, including async returns, error unions, and imported types.

Unannotated `catch (error)` bindings receive the inferred errors escaping their `try` body. Native TypeScript checks (`instanceof`, `typeof`, and discriminants) narrow that union; narrowed rethrows also refine `Returns` checking. Awaited rejections are included, while unawaited promises are not. Unknown effects keep the binding `unknown`, and explicit `any`/`unknown` catch annotations are preserved.

Inline `.catch(error => ...)` and `.then(onFulfilled, error => ...)` callbacks also receive inferred rejection types, including errors added by earlier handlers in the chain. Their parameters support ordinary TypeScript narrowing and update after unsaved dependency edits.

## Build and install

From `subs/errs-vscode/extensions/typescript-language-features`:

```sh
mise :build:dev
mise :build/package
mise :test/editor
```

The equivalent commands without mise are `node scripts/errs/build.mjs --dev` and `node scripts/errs/package.mjs`; Node, npm, and Cargo must be on PATH. The build installs the module's locked npm dependencies and runs its focused TypeScript check. Rust uses the fork's pinned toolchain and original dev settings. Set `ERRS_REPO` when the compiler checkout is somewhere other than the sibling `subs/errs`.

The editor test uses an installed `code` executable (or `VSCODE_EXECUTABLE`) and an isolated profile/workspace. It verifies language selection, hover links and their opened source positions, standard-library Go to Definition, unsaved JS dependency edits, and restart/reselection. On headless Linux, after a dev build run `node scripts/errs/test.mjs --disable-gpu --no-sandbox --ozone-platform=headless`.

Install `dist/errorscript-<platform>-<arch>.vsix` using **Extensions: Install from VSIX**. Its ID is **nocorp.errorscript**. The package includes the compiler and matching `lib*.d.ts` files in `server/libs`, copied from `subs/errs/crates/tsrs_vfs/libs`. The client sets the server's existing `TSRS_LIB_PATH` override to this physical directory so VS Code can open declarations. Standalone tsrs embeds the same libraries under `bundled:///libs` for compilation without external files; npm's `noembed` builds instead ship them beside the executable. No TypeScript npm installation, Cargo, or repository checkout is needed to use the installed extension. The VSIX targets the machine where it was built. `errorscript.server.path` can select a different fork binary (using the extension's packaged libraries); reload the extension after changing it. **ErrorScript: Restart Language Server** restarts the current binary.

## Upstream merge boundaries

- Compiler semantics: `subs/errs/crates/errs`, linked hover rendering in `crates/errs_ls`, with a native adapter in `tsrs_checker/src/errs.rs` and small marked hooks. See `subs/errs/docs/ERRORSCRIPT.md` for the API, tests, and current inference limits.
- Extension: `src/errs` contains the client, standalone dependencies, and contributions; `scripts/errs` contains build/VSIX tooling. The upstream extension entry point and its large contribution manifest remain available as merge references. The generated development/production manifests select only ErrorScript contributions and the isolated entry point, avoiding duplicate built-in TypeScript commands/configuration.
- Launch and tasks: this repository's `.vscode`, following the Genotype prelaunch-task layout. The sparse VS Code checkout is sufficient; builds read its tracked TypeScript language configuration with `git show` and reuse VS Code's built-in grammar scopes.

Inference supplies checked explicit return contracts and optional root/missing-annotation diagnostics; effects impose no general function-value assignability constraints. Unknown implementations stay `unknown`; `never` is relative to the supported model, not a claim that arbitrary JavaScript cannot fail. See the compiler's scope/limitations for unsupported operations.
