# ErrorScript LSP

A dummy `tower-lsp` server with Tower concurrency middleware, exposed to Node.js
via napi-rs. It is independent of `pkgs/crate`.

```sh
pnpm --filter @errorscript/lsp build
pnpm --filter @errorscript/lsp check
pnpm --filter @errorscript/lsp test
node pkgs/lsp/dist/server.cjs
```

The build generates the Node.js binding, TypeScript declarations, and native
binary in `dist`. The launcher uses LSP-framed stdio; run it in a dedicated child
process. Stdout is reserved for protocol messages.

The launcher and build script are written in TypeScript. Tests use Vitest and
exercise the compiled native server over stdio.

The server supports full document synchronization, dummy hover information,
and informational diagnostics on lines containing `errorscript-dummy`.
Diagnostics use UTF-16 positions and are cleared on edits and document close.
