# ErrorScript

TypeScript/JavaScript error inference and checked `Returns<Value, Error>` contracts, with compiler, editor, and built-in API evidence tooling.

- `pkgs/bin`: compiler CLI packaging and releases.
- `pkgs/lib`: declaration inventory, error repro tests, and annotation evidence.
- `pkgs/vscode`: ErrorScript VS Code extension packaging.
- `pkgs/website`: errorscript.org (Astro).
- `subs/errs`: Rust TypeScript compiler/language-server fork with error inference.
- `subs/errs-vscode`: VS Code fork supplying the editor integration.

## Tasks

Never define tasks in `package.json` scripts. Always use the package's `mise.toml`.
