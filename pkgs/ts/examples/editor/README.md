# ErrorScript TypeScript 7 editor PoC

From the repository root, run `pnpm install`, then open this directory in VS Code:

```sh
code pkgs/ts/examples/editor
```

Press **F5** to launch **ErrorScript TypeScript 7 PoC**. In the Extension
Development Host, open `sample/index.ts` (the sample folder is opened automatically).
The language mode should be **ErrorScript (TypeScript 7 PoC)**.

- Hover the catch variable: its virtual type is `any`.
- Inspect the intentional assignment error, then fix it without saving.
- Try `catch (error: unknown)` to get normal TypeScript checking for that binding.
- Run **ErrorScript TS7: Show Transformed Source** to inspect `catch (error: errtype)`
  and the appended alias beside the original source.

This extension uses the native TypeScript 7.1 API through the server in
`../../src/editor-server.js`. It does not use the legacy TypeScript 6 plugin.
The prototype provides diagnostics and hover; other semantic editor features
are not implemented yet. See [the package README](../../README.md) for architecture
and configuration details.
