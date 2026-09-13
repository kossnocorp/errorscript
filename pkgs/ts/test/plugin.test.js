import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const serverPath = require.resolve("typescript-legacy/lib/tsserver.js");
const packagePath = fileURLToPath(new URL("..", import.meta.url));
const source = `export {}; /* 🐛 */ try {} catch (error) { error.message; } const wrong: number = "oops";`;

/** @typedef {import("typescript-legacy").server.protocol.Response} Response */

/**
 * Use the real tsserver so plugin discovery, unsaved buffers, and protocol
 * coordinate conversion are exercised together.
 * @param {import("node:test").TestContext} t
 * @param {boolean} enabled
 */
function startServer(t, enabled) {
  const directory = mkdtempSync(join(tmpdir(), "errorscript-plugin-"));
  const file = join(directory, "index.ts");
  mkdirSync(join(directory, "node_modules/@errorscript"), { recursive: true });
  symlinkSync(packagePath, join(directory, "node_modules/@errorscript/ts"), "junction");
  writeFileSync(file, source);
  writeFileSync(
    join(directory, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        strict: true,
        noEmit: true,
        types: [],
        plugins: enabled ? [{ name: "@errorscript/ts" }] : [],
      },
      files: ["index.ts"],
    }),
  );
  const child = spawn(
    process.execPath,
    [serverPath, "--allowLocalPluginLoads", "--disableAutomaticTypingAcquisition"],
    {
      cwd: directory,
      stdio: "pipe",
    },
  );
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  let sequence = 0;
  /** @type {Map<number, (response: Response) => void>} */
  const pending = new Map();
  const lines = createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    // tsserver emits Content-Length headers, followed by single-line JSON.
    if (!line.startsWith("{")) return;
    const response = JSON.parse(line);
    if (response.type !== "response") return;
    pending.get(response.request_seq)?.(response);
    pending.delete(response.request_seq);
  });
  t.after(async () => {
    const exited = new Promise((resolve) => child.once("exit", resolve));
    child.kill();
    if (child.exitCode === null && child.signalCode === null) await exited;
    lines.close();
    rmSync(directory, { recursive: true, force: true });
  });

  /**
   * @param {string} command
   * @param {object} args
   * @returns {Promise<Response>}
   */
  function request(command, args) {
    const seq = ++sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(seq);
        reject(new Error(`tsserver timed out: ${command}\n${stderr}`));
      }, 10_000);
      pending.set(seq, (response) => {
        clearTimeout(timer);
        if (response.success) resolve(response);
        else reject(new Error(JSON.stringify(response)));
      });
      child.stdin.write(JSON.stringify({ seq, type: "request", command, arguments: args }) + "\n");
    });
  }
  return { file, request };
}

test("legacy tsserver without the plugin checks the original catch binding", async (t) => {
  const { file, request } = startServer(t, false);
  await request("open", { file });
  const { body } = await request("semanticDiagnosticsSync", { file });
  assert.deepEqual(
    body.map(/** @param {{ code: number }} diagnostic */ (diagnostic) => diagnostic.code).sort(),
    [18046, 2322].sort(),
  );
});

test("plugin loads by package name and maps diagnostics, hover, rename, and unsaved changes", async (t) => {
  const { file, request } = startServer(t, true);
  await request("open", { file });
  const { body: diagnostics } = await request("semanticDiagnosticsSync", { file });
  assert.equal(diagnostics.length, 1, JSON.stringify(diagnostics));
  assert.equal(diagnostics[0].code, 2322);
  assert.deepEqual(diagnostics[0].start, { line: 1, offset: source.indexOf("wrong") + 1 });

  const offset = source.indexOf("error.message") + 1;
  const { body: hover } = await request("quickinfo", { file, line: 1, offset });
  // TypeScript expands aliases of any in quick info, even though the virtual
  // source contains the explicit errtype annotation.
  assert.match(hover.displayString, /error: any/);
  assert.deepEqual(hover.start, { line: 1, offset });

  const { body: rename } = await request("rename", { file, line: 1, offset });
  assert.equal(rename.info.canRename, true);
  assert.deepEqual(
    rename.locs[0].locs
      .map(/** @param {{ start: { offset: number } }} loc */ (loc) => loc.start.offset)
      .sort(),
    [source.indexOf("error)") + 1, offset].sort(),
  );

  // A new snapshot must be transformed again, with fresh mappings. An explicit
  // unknown annotation must remain unknown rather than being silenced.
  const updated = `export {}; try {} catch (changed: unknown) { changed.message; }`;
  await request("updateOpen", {
    changedFiles: [
      {
        fileName: file,
        textChanges: [
          {
            start: { line: 1, offset: 1 },
            end: { line: 1, offset: source.length + 1 },
            newText: updated,
          },
        ],
      },
    ],
  });
  const { body: afterEdit } = await request("semanticDiagnosticsSync", { file });
  assert.equal(afterEdit.length, 1, JSON.stringify(afterEdit));
  assert.equal(afterEdit[0].code, 18046);
  assert.deepEqual(afterEdit[0].start, { line: 1, offset: updated.indexOf("changed.message") + 1 });
  assert.equal(readFileSync(file, "utf8"), source);
});
