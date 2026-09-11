# ErrorScript for VS Code

Dummy ErrorScript language features for JavaScript, JSX, TypeScript, and TSX.
The extension starts a separate Rust language server through the generated
`@errorscript/lsp` napi-rs package. VS Code's built-in JS/TS support stays active;
ErrorScript adds its own hover and diagnostic collection.

Hover in a JS/TS document to see the dummy server status. Add
`// errorscript-dummy` to see an informational diagnostic. Remove the marker to
clear it. There is no compiler integration yet.

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
