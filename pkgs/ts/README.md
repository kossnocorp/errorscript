# @errorscript/ts — content mapper PoC

Temporary regex-based experiment with [TypeScript content mappers](https://github.com/microsoft/typescript-go/pull/4712).
The workspace pins `typescript@7.1.0-dev.20260913.1` (the `next` build used for this experiment).

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

## Try it

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

`src/transform.js` inserts `: errtype` after bare catch identifiers:

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
- When iterating with incremental/build mode, bump the mapper package version or
  force a rebuild to invalidate cached transformations.

Set `TS_CONTENT_MAPPER_DEBUG=1` when running the example to inspect protocol traffic
and the generated text.
