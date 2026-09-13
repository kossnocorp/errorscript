# ErrorScript LSP

A `tower-lsp` server exposed to Node.js via napi-rs, using the shared ErrorScript
compiler in `pkgs/crate`.

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

The server supports full document synchronization, call-site error hovers, and
error-severity `uncaught-call` diagnostics for unhandled module-level calls.
Diagnostics and hover ranges use UTF-16 positions, including CRLF and non-BMP
characters. Open buffers override disk contents; closing a buffer restores disk
analysis for its dependents and clears that document's diagnostics.

The editor builder discovers configured roots (`errconfig.toml` / `tsconfig.json`),
open documents, and transitive runtime/type dependencies. File notifications also
refresh import resolution, so creating or deleting a dependency updates callers.
Unchanged modules retain their semantic arenas and call reports. Changed modules
invalidate their connected import component: both directions are necessary because
function argument inference propagates caller information into callees. Global
mutations conservatively invalidate all modules. Function graph IDs are rebuilt
within the affected snapshot rather than reused across edits.

Compiler work runs on a blocking task with exclusively owned AST arenas. A single
background worker coalesces rapid edits and publishes only the newest revision.
LSP notifications and request cancellation remain responsive during analysis;
pending hovers wake when the latest snapshot is ready. Stale document versions
and duplicate disk-change notifications from saving open buffers are ignored.
