import { cp, mkdir, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { build } from "esbuild";

const require = createRequire(import.meta.url);

async function main() {
  await rm("dist", { recursive: true, force: true });
  await mkdir("dist/lsp", { recursive: true });
  await build({
    entryPoints: ["src/extension.ts"],
    outfile: "dist/extension.cjs",
    bundle: true,
    platform: "node",
    format: "cjs",
    target: "node20",
    external: ["vscode"],
  });
  const server = require.resolve("@errorscript/lsp/server");
  await cp(path.dirname(server), "dist/lsp", {
    recursive: true,
  });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
