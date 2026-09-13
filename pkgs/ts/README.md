# @errorscript/ts — plugin and content mapper PoC

Three temporary adapters around the same regex-based source transform:

- **TypeScript 7 editor:** ordinary `.ts` files in a standalone API-backed language server and VS Code extension.
- **Language-service plugin:** ordinary `.ts` files in the legacy TypeScript 6 editor server.
- **Content mapper:** `.ets` files with [TypeScript 7.1 content mappers](https://github.com/microsoft/typescript-go/pull/4712).

All insert `catch (name: errtype)` and append the documented `type errtype = any`.
The alias is a placeholder for experimenting with future ErrorScript-inferred catch types.
The workspace pins `typescript@7.1.0-dev.20260913.1`; this package additionally installs
`typescript-legacy` as an alias for TypeScript 6.0.3 for the plugin experiment.

## Try the TypeScript 7 editor prototype

From the repository root:

```sh
pnpm install
code pkgs/ts/examples/editor
```

In that VS Code window, press **F5** and choose **ErrorScript TypeScript 7 PoC**.
The Extension Development Host opens the `sample` folder with the prototype
extension loaded. No build step is needed; the server uses `node` on PATH
(the repository's Node 24 environment works).

Open `index.ts` in the development window:

1. The status bar should say **ErrorScript (TypeScript 7 PoC)**.
2. `error.message` has no unknown-catch error; hovering `error` shows `any`.
3. The intentional `wrong: number = "oops"` error is underlined at its original position.
4. Change a catch to `catch (error: unknown)` to see checking change immediately,
   without saving. Changing an imported open file also updates its importers.
5. Run **ErrorScript TS7: Show Transformed Source** to open the virtual TypeScript
   beside the original. Run the command again after edits to get a fresh view.

This uses **TypeScript 7.1.0-dev.20260913.1**, independently of VS Code's selected
TypeScript version. Its output channel is **ErrorScript TypeScript 7 PoC**.

### How it fits together

```text
VS Code .ts editor buffers
  → ErrorScript LSP server
  → TypeScript 7 API filesystem callback
  → shared transform: catch annotations + errtype alias
  → native TypeScript type checking / type queries
  → positions mapped back to the original buffers
  → diagnostics and hover in VS Code
```

`src/editor-project.js` creates the API program, overlays all open buffers, and
transforms project source reads. `src/editor-server.js` exposes diagnostics,
hover, and virtual-source inspection over LSP. The small extension lives in
`examples/editor`.

The nearest `tsconfig.json` supplies compiler options and root files. Enable
the transform by putting this entry directly in that config:

```json
{
  "compilerOptions": {
    "strict": true,
    "moduleDetection": "force",
    "plugins": [{ "name": "@errorscript/ts" }]
  }
}
```

**Our server interprets that entry. Native `tsc` and the stock TypeScript 7
language server do not load it.** Removing the entry makes our server check the
original source. For a loose file with no tsconfig, ErrorScript mode enables
the transform with strict defaults. Declaration files and `node_modules` are
not transformed.

The sample's `.vscode/settings.json` associates `*.ts` with our dedicated
language mode. The files retain their `.ts` extension, but this mode gives the
prototype ownership of editor diagnostics and hover instead of receiving
conflicting results from stock TypeScript providers. It reuses TypeScript
syntax highlighting. To test another folder in the Development Host, add the
same file association and plugin entry there.

### Editor PoC scope

- Implemented: live syntactic/semantic diagnostics, type hover, source mapping,
  unsaved buffers across imports, file-change refresh, and virtual-source inspection.
- Completions, navigation, rename, code actions, and formatting are not implemented
  in this TS 7 adapter yet. This is not a full replacement for the TypeScript editor service.
- Each operation creates a fresh API program/process. Changes are debounced;
  incremental snapshots and project caching are future architecture work.
- This is a single-project experiment, not a solution-build host. It reads the
  plugin entry directly from the nearest config rather than implementing a general
  plugin loader or inherited plugin configuration.
- Entirely generated-code diagnostics are surfaced at the start of the document
  with an explanatory message. Hover displays the checker type (`any` for now);
  **Show Transformed Source** shows the actual `errtype` alias.

Run the real LSP integration tests and extension type-check:

```sh
pnpm --filter @errorscript/ts test:editor
pnpm --filter errorscript-ts7-editor-poc check
```

## Try the language-service plugin on `.ts`

Install workspace dependencies with `pnpm install`, then open
`pkgs/ts/example-plugin/errorscript.code-workspace` in VS Code. Run
**TypeScript: Select TypeScript Version → Use Workspace Version** and confirm
the selected version is **6.0.3**. Use the legacy TypeScript editor service for
this workspace, disabling the native TypeScript 7 extension here if necessary.
Run **TypeScript: Restart TS Server** after editing the plugin implementation.

The example uses this configuration:

```json
{
  "compilerOptions": {
    "strict": true,
    "moduleDetection": "force",
    "plugins": [{ "name": "@errorscript/ts" }]
  }
}
```

Open `index.ts`: `error.message` should have no unknown-catch diagnostic.
Hovering `error` displays `any` (TypeScript expands the alias in quick info).
Uncomment the `wrong` declaration to see an ordinary assignment diagnostic.
Rename and hover should use the original source positions, and unsaved edits
should be transformed immediately.

The plugin entry is `src/plugin.cjs`. It uses Volar's language-service adapter
to supply transformed snapshots and map editor requests and results, reusing
the same verbatim mappings as the content mapper. Only `.ts` files outside
`node_modules`, excluding `.d.ts`, are transformed.

This is an editor plugin: plain `tsc` does not load it. It is not a plugin for
the TypeScript 7 native language server. Test it without an editor using:

```sh
pnpm --filter @errorscript/ts test:plugin
```

These tests launch the real TypeScript 6 tsserver, load `@errorscript/ts` from
tsconfig, and exercise diagnostics, hover, rename, and unsaved edits.

## Important finding: `.ts` cannot be registered

The requested top-level tsconfig setting:

```json
{
  "contentMappers": [
    {
      "package": "@errorscript/ts",
      "extensions": [".ts"]
    }
  ]
}
```

is rejected by this compiler with **TS100021: Content mapper file extension '.ts' is a built-in extension and cannot be registered by a content mapper.**
Supporting ordinary `.ts` files through this mechanism would require an upstream change or a compiler fork.
The runnable example uses `.ets` to exercise the same transform through the supported API.

## Try the content mapper

From the repository root:

```sh
pnpm install
pnpm exec tsc --version
pnpm --filter @errorscript/ts example
pnpm --filter @errorscript/ts test
pnpm --filter @errorscript/ts check
```

The root workspace links `@errorscript/ts` so tsconfig package resolution can find it.
For another workspace consumer, add `"@errorscript/ts": "workspace:*"` to its dev dependencies.
Use `"extensions": [".ets"]` and include those files in the project's tsconfig.
Run `tsc --project <project> --noEmit --runExternalCode` with the 7.1 next compiler.
The mapper runs directly as JavaScript, so it needs no build step.

## Transform

`src/transform.cjs` contains the shared transform; `src/transform.js` re-exports it
for the ESM mapper process. It inserts `: errtype` after bare catch identifiers:

```ts
try {
  throw new Error("Example failure");
} catch (error: errtype) {
  error.message;
}

/**
 * ErrorScript-typed error. It is `any` alias to disable TypeScript and let
 * the ErrorScript type system to do the type checking.
 */
type errtype = any;
```

`src/server.js` serves the four content mapper methods over stdio JSON-RPC.
Verbatim UTF-16 span mappings preserve diagnostics and language-service positions
in original content; inserted annotations and the appended alias are unmapped.
Only virtual compiler content changes; source files on disk are untouched.

## PoC boundaries

- The regex recognizes ASCII identifier bindings, whitespace, and multiple catches.
  Already annotated and bindingless catches are left alone.
- It does not parse TypeScript: matching text in comments, strings, or regex literals
  can be rewritten. Destructuring, escaped identifiers, and Unicode identifiers are unsupported.
- The alias is appended to every transformed file. Existing `errtype` declarations
  can collide; use modules (the example sets `moduleDetection: "force"`) to avoid
  sharing aliases across global scripts.
- This disables TypeScript checking of catch bindings; it does not implement
  ErrorScript's own type checking.
- Content-mapped files are not emitted as JavaScript by TypeScript. This experiment
  uses `noEmit`.
- The plugin delegates position mapping to Volar, but this is not a full editor
  integration test suite. Formatting is not enabled for its generated mappings,
  and diagnostics entirely in synthesized code are not surfaced by that adapter.
- When iterating with incremental/build mode, bump the mapper package version or
  force a rebuild to invalidate cached transformations.

Set `TS_CONTENT_MAPPER_DEBUG=1` when running the example to inspect protocol traffic
and the generated text.
