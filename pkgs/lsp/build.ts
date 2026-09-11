import { writeFile } from "node:fs/promises";
import { build } from "esbuild";

// napi-rs generates CommonJS bindings even though our sources use ESM.
await writeFile("dist/package.json", JSON.stringify({ type: "commonjs" }));
await build({
  entryPoints: ["server.ts"],
  outfile: "dist/server.cjs",
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  packages: "external",
  external: ["*.node"],
});
