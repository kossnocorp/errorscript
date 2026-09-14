# ErrorScript for VS Code

ErrorScript language features for JavaScript, JSX, TypeScript, and TSX.
The extension starts a separate Rust language server through the generated
`@errorscript/lsp` napi-rs package. VS Code's built-in JS/TS support stays active;
ErrorScript adds its own hover and diagnostic collection.

Install the extension and open a JS/TS file to start analysis automatically. Hover
over a function call to see the errors it may throw or reject with, including
errors propagated through imported functions.

Unhandled **module-level calls** get a red underline and an ErrorScript entry in
the Problems panel:

```ts
function load() {
  throw new TypeError("Invalid input");
}
async function save() {
  throw new RangeError("Out of range");
}

load(); // ErrorScript: TypeError
save(); // ErrorScript: RangeError

try {
  load();
} catch (error) {} // Handled synchronous error
try {
  await save();
} catch (error) {} // Handled rejection
save().catch(() => {}); // Handled rejection
```

A `try/catch` around an unawaited async call does not handle its rejection.
Rejection handlers supplied to `.then()` are also recognized; `.finally()` alone
does not handle rejections. Errors thrown by a promise handler remain errors of
the resulting promise. Calls inside functions contribute to their function's
error summary and still have hover information.

Analysis updates while you type, including unsaved changes in dependencies, and
also responds to file creation, deletion, and disk edits. ErrorScript reads
`errconfig.toml` or `tsconfig.json` and follows imports from open documents.
Unrelated parsed modules and reports are retained across edits.

This is a conservative may-throw analysis: `unknown` in a hover means the analyzer
cannot resolve all possible errors, for example from dynamic dispatch. Stored
promises are recognized when all uses are known handlers or caught awaits;
arbitrary promise escapes remain conservative.

## Development

From the repository root:

```sh
pnpm install
pnpm --filter errorscript-vscode build
pnpm --filter errorscript-vscode check
code --extensionDevelopmentPath="$PWD/pkgs/vscode"
```

Run `pnpm --filter errorscript-vscode package` to create a VSIX. The build bundles
the language client and copies the generated native LSP package into the
extension, so installation does not require workspace dependencies. The native
binary is specific to the build machine's OS and architecture; build a separate
VSIX on each target platform (and use `vsce package --no-dependencies --target`
with the corresponding VS Code target when distributing).

## Catch cast checking

Enable this opt-in check in `errconfig.toml`:

```toml
[checks]
cast_catch = true
```

For a catch that accesses its caught error and has known inferred error types, ErrorScript requires the parameter
name `err_` and a first statement declaring `const err = err_ as <type>`:

```ts
try {
  throw new Error("Wut");
} catch (err_) {
  const err = err_ as Error;
}
```

The asserted type must match the recorded error types; unions may appear in any
order. Incorrect types, missing casts, and incorrect binding names produce an
error diagnostic (`cast-catch`). “First statement” ignores comments and whitespace.
Handlers that never read the caught binding, including `catch {}` and
`catch (err_) {}`, are exempt. Reads inside closures count; unrelated shadowed
bindings do not. Destructuring a catch parameter counts as accessing the error.
Catches with no recorded errors, `unknown`, or any union containing `unknown`
are skipped entirely. Files under a `node_modules` path component are external:
their errors still contribute to inference, but this check does not enforce
catch conventions in those files. Configuration and dependency edits refresh
the diagnostics automatically. The CLI `build` command also reports violations
and exits unsuccessfully.
