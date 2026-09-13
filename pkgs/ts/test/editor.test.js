import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  createMessageConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from "vscode-jsonrpc/node.js";

/** @typedef {import("vscode-languageserver").PublishDiagnosticsParams} Published */

/** @param {import("node:test").TestContext} t */
async function startEditor(t) {
  const directory = mkdtempSync(join(tmpdir(), "errorscript-editor-"));
  writeFileSync(
    join(directory, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        strict: true,
        module: "nodenext",
        moduleDetection: "force",
        types: [],
        plugins: [{ name: "@errorscript/ts" }],
      },
      include: ["*.ts"],
    }),
  );
  const child = spawn(
    process.execPath,
    [fileURLToPath(new URL("../src/editor-server.js", import.meta.url))],
    { stdio: "pipe" },
  );
  const connection = createMessageConnection(
    new StreamMessageReader(child.stdout),
    new StreamMessageWriter(child.stdin),
  );
  let errors = "";
  child.stderr.on("data", (chunk) => {
    errors += chunk;
  });
  /** @type {Set<(params: Published) => void>} */
  const listeners = new Set();
  connection.onNotification(
    "textDocument/publishDiagnostics",
    /** @param {Published} params */ (params) => {
      for (const listener of listeners) listener(params);
    },
  );
  connection.listen();
  t.after(async () => {
    const exited = new Promise((resolve) => child.once("exit", resolve));
    child.kill();
    if (child.exitCode === null && child.signalCode === null) await exited;
    connection.dispose();
    rmSync(directory, { recursive: true, force: true });
  });
  /** @type {import("vscode-languageserver").InitializeResult} */
  const initialized = await connection.sendRequest("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(directory).href,
    capabilities: {},
  });
  assert.equal(initialized.capabilities.hoverProvider, true);
  assert.equal(initialized.serverInfo?.name, "ErrorScript TypeScript 7 PoC");
  await connection.sendNotification("initialized", {});

  /** @param {string} uri @param {(params: Published) => boolean} predicate */
  function diagnostics(uri, predicate) {
    return new Promise(
      /** @param {(params: Published) => void} resolve */ (resolve, reject) => {
        const timer = setTimeout(() => {
          listeners.delete(listener);
          reject(new Error(`Diagnostics timed out for ${uri}\n${errors}`));
        }, 10_000);
        /** @param {Published} params */
        function listener(params) {
          if (params.uri !== uri || !predicate(params)) return;
          clearTimeout(timer);
          listeners.delete(listener);
          resolve(params);
        }
        listeners.add(listener);
      },
    );
  }
  return { directory, connection, diagnostics };
}

test(
  "TS7 editor maps diagnostics and hover, exposes virtual source, and updates unsaved buffers",
  { timeout: 30_000 },
  async (t) => {
    const { directory, connection, diagnostics } = await startEditor(t);
    const file = join(directory, "index.ts");
    const uri = pathToFileURL(file).href;
    const source = `export {}; /* 🐛 */ try {} catch (error) { error.message; } const wrong: number = "oops";`;
    writeFileSync(file, source);
    let result = diagnostics(uri, (params) => params.version === 1);
    await connection.sendNotification("textDocument/didOpen", {
      textDocument: { uri, languageId: "errorscript-typescript", version: 1, text: source },
    });
    const published = await result;
    assert.deepEqual(
      published.diagnostics.map((d) => d.code),
      [2322],
    );
    assert.equal(published.diagnostics[0]?.range.start.character, source.indexOf("wrong"));
    assert.equal(published.diagnostics[0]?.source, "errorscript-ts7");
    /** @type {import("vscode-languageserver").Hover} */
    const hover = await connection.sendRequest("textDocument/hover", {
      textDocument: { uri },
      position: { line: 0, character: source.indexOf("error.message") },
    });
    assert.match(JSON.stringify(hover.contents), /error: any/);
    assert.equal(hover.range?.start.character, source.indexOf("error.message"));
    const virtual = await connection.sendRequest("errorscript/virtualDocument", { uri });
    assert.equal(typeof virtual, "string");
    assert.match(/** @type {string} */ (virtual), /catch \(error: errtype\)/);
    assert.match(/** @type {string} */ (virtual), /type errtype = any/);

    // A tsconfig edit must change behavior without restarting the editor server.
    const configFile = join(directory, "tsconfig.json");
    const enabledConfig = readFileSync(configFile, "utf8");
    const disabledConfig = JSON.parse(enabledConfig);
    disabledConfig.compilerOptions.plugins = [];
    writeFileSync(configFile, JSON.stringify(disabledConfig));
    result = diagnostics(uri, (params) => params.diagnostics.some((d) => d.code === 18046));
    await connection.sendNotification("workspace/didChangeWatchedFiles", {
      changes: [{ uri: pathToFileURL(configFile).href, type: 2 }],
    });
    await result;
    writeFileSync(configFile, enabledConfig);
    result = diagnostics(
      uri,
      (params) => params.diagnostics.length === 1 && params.diagnostics[0]?.code === 2322,
    );
    await connection.sendNotification("workspace/didChangeWatchedFiles", {
      changes: [{ uri: pathToFileURL(configFile).href, type: 2 }],
    });
    await result;

    const updated = `export {}; /* 🐛 */ try {} catch (changed: unknown) { changed.message; }`;
    result = diagnostics(uri, (params) => params.version === 2);
    await connection.sendNotification("textDocument/didChange", {
      textDocument: { uri, version: 2 },
      contentChanges: [{ text: updated }],
    });
    const changed = await result;
    assert.deepEqual(
      changed.diagnostics.map((d) => d.code),
      [18046],
    );
    assert.equal(changed.diagnostics[0]?.range.start.character, updated.indexOf("changed.message"));
    const updatedHover = await connection.sendRequest("textDocument/hover", {
      textDocument: { uri },
      position: { line: 0, character: updated.indexOf("changed.message") },
    });
    assert.match(JSON.stringify(updatedHover), /changed: unknown/);
    assert.equal(readFileSync(file, "utf8"), source);

    result = diagnostics(uri, (params) => params.version === 3);
    await connection.sendNotification("textDocument/didChange", {
      textDocument: { uri, version: 3 },
      contentChanges: [{ text: "export const value = ;" }],
    });
    assert.ok((await result).diagnostics.some((d) => d.code === 1109));
    result = diagnostics(uri, (params) => params.diagnostics.length === 0);
    await connection.sendNotification("textDocument/didClose", { textDocument: { uri } });
    await result;
    await connection.sendRequest("shutdown");
    await connection.sendNotification("exit");
  },
);

test(
  "TS7 editor rechecks importers when another unsaved buffer changes or closes",
  { timeout: 30_000 },
  async (t) => {
    const { directory, connection, diagnostics } = await startEditor(t);
    const uri = pathToFileURL(join(directory, "index.ts")).href;
    const dependencyUri = pathToFileURL(join(directory, "dependency.ts")).href;
    const source = 'import { value } from "./dependency.js"; export const wrong: number = value;';
    writeFileSync(join(directory, "index.ts"), source);
    writeFileSync(join(directory, "dependency.ts"), 'export const value = "oops";');
    let result = diagnostics(uri, (params) => params.diagnostics.some((d) => d.code === 2322));
    await connection.sendNotification("textDocument/didOpen", {
      textDocument: { uri, languageId: "errorscript-typescript", version: 1, text: source },
    });
    await result;
    result = diagnostics(uri, (params) => params.diagnostics.length === 0);
    await connection.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri: dependencyUri,
        languageId: "errorscript-typescript",
        version: 1,
        text: "export const value = 123;",
      },
    });
    await result;
    result = diagnostics(uri, (params) => params.diagnostics.some((d) => d.code === 2322));
    await connection.sendNotification("textDocument/didClose", {
      textDocument: { uri: dependencyUri },
    });
    await result;
    await connection.sendRequest("shutdown");
    await connection.sendNotification("exit");
  },
);
