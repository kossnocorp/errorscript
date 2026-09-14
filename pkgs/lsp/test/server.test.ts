import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath, pathToFileURL } from "node:url";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";

interface Message {
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code: number };
}

test("native stdio server lifecycle, hover, and diagnostics", async ({ onTestFinished }) => {
  const directory = await mkdtemp(join(tmpdir(), "errorscript-lsp-"));
  onTestFinished(() => rm(directory, { recursive: true, force: true }));
  await writeFile(join(directory, "errconfig.toml"), 'files = ["*.ts"]');
  await writeFile(
    join(directory, "worker.ts"),
    "export function fail() { throw new TypeError(); }",
  );
  const text = 'import { fail } from "./worker";\r\n/* 😀 */ fail();\r\n';
  await writeFile(join(directory, "entry.ts"), text);
  const child = spawn(
    process.execPath,
    [fileURLToPath(new URL("../dist/server.cjs", import.meta.url))],
    {
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  onTestFinished(() => {
    child.kill();
  });
  let stderr = "";
  child.stderr.on("data", (chunk: Buffer) => {
    stderr += chunk.toString();
  });
  let buffer: Buffer = Buffer.alloc(0);
  const messages: Message[] = [];
  let wake: (() => void) | undefined;
  child.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) break;
      const header = buffer.subarray(0, headerEnd).toString();
      const length = Number(/Content-Length: (\d+)/i.exec(header)?.[1]);
      expect(Number.isFinite(length), `Invalid LSP header: ${header}`).toBe(true);
      if (buffer.length < headerEnd + 4 + length) break;
      messages.push(
        JSON.parse(buffer.subarray(headerEnd + 4, headerEnd + 4 + length).toString()) as Message,
      );
      buffer = buffer.subarray(headerEnd + 4 + length);
      wake?.();
    }
  });
  function send(message: Message) {
    const body = JSON.stringify({ jsonrpc: "2.0", ...message });
    child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
  }
  async function receive(predicate: (message: Message) => boolean): Promise<Message> {
    while (true) {
      const index = messages.findIndex(predicate);
      if (index >= 0) return messages.splice(index, 1)[0]!;
      await new Promise<void>((resolve) => {
        wake = resolve;
      });
    }
  }
  const uri = pathToFileURL(join(directory, "entry.ts")).href;
  const workerUri = pathToFileURL(join(directory, "worker.ts")).href;
  const highlights = (version: number, empty = false) =>
    receive((message) => {
      const params = message.params as { uri: string; version: number; ranges: unknown[] };
      return (
        message.method === "errorscript/throwingCalls" &&
        params.uri === uri &&
        params.version === version &&
        (!empty || params.ranges.length === 0)
      );
    });
  const diagnostic = (target = uri, version?: number) =>
    receive(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        (message.params as { uri: string }).uri === target &&
        (version === undefined || (message.params as { version: number }).version === version),
    );
  send({
    id: 1,
    method: "initialize",
    params: { processId: process.pid, rootUri: pathToFileURL(directory).href, capabilities: {} },
  });
  expect((await receive((message) => message.id === 1)).result).toMatchObject({
    serverInfo: { name: "ErrorScript" },
    capabilities: { textDocumentSync: 1 },
  });
  send({ method: "initialized", params: {} });
  send({
    method: "textDocument/didOpen",
    params: {
      textDocument: {
        uri,
        languageId: "typescript",
        version: 1,
        text,
      },
    },
  });
  expect((await diagnostic()).params).toMatchObject({
    uri,
    version: 1,
    diagnostics: [
      {
        source: "ErrorScript",
        severity: 1,
        code: "uncaught-call",
        message: expect.stringContaining("TypeError"),
        range: {
          start: { line: 1, character: 9 },
          end: { line: 1, character: 15 },
        },
      },
    ],
  });
  expect((await highlights(1)).params).toEqual({
    uri,
    version: 1,
    ranges: [{ start: { line: 1, character: 9 }, end: { line: 1, character: 13 } }],
  });
  send({
    id: 2,
    method: "textDocument/hover",
    params: {
      textDocument: { uri },
      position: { line: 1, character: 10 },
    },
  });
  expect((await receive((message) => message.id === 2)).result).toMatchObject({
    contents: { value: expect.stringContaining("TypeError") },
  });
  send({
    id: 4,
    method: "textDocument/hover",
    params: {
      textDocument: { uri },
      position: { line: 0, character: 0 },
    },
  });
  expect((await receive((message) => message.id === 4)).result).toBeNull();
  send({
    method: "textDocument/didOpen",
    params: {
      textDocument: {
        uri: workerUri,
        languageId: "typescript",
        version: 1,
        text: "export function fail() { throw new RangeError(); }",
      },
    },
  });
  expect((await diagnostic()).params).toMatchObject({
    diagnostics: [{ message: expect.stringContaining("RangeError") }],
  });
  await diagnostic(workerUri, 1);
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri: workerUri, version: 2 },
      contentChanges: [{ text: "export function fail() {}" }],
    },
  });
  expect((await diagnostic()).params).toMatchObject({ diagnostics: [] });
  await diagnostic(workerUri, 2);
  expect((await highlights(1, true)).params).toMatchObject({ ranges: [] });
  send({ method: "textDocument/didClose", params: { textDocument: { uri: workerUri } } });
  expect((await diagnostic()).params).toMatchObject({
    diagnostics: [{ message: expect.stringContaining("TypeError") }],
  });
  await diagnostic(workerUri);
  await writeFile(
    join(directory, "worker.ts"),
    "export function fail() { throw new SyntaxError(); }",
  );
  send({
    method: "workspace/didChangeWatchedFiles",
    params: { changes: [{ uri: workerUri, type: 2 }] },
  });
  expect((await diagnostic()).params).toMatchObject({
    diagnostics: [{ message: expect.stringContaining("SyntaxError") }],
  });
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 2 },
      contentChanges: [{ text: 'import { fail } from "./worker"; try { fail(); } catch {}' }],
    },
  });
  expect((await diagnostic()).params).toEqual({ uri, version: 2, diagnostics: [] });
  expect((await highlights(2)).params).toMatchObject({
    ranges: [{ start: { line: 0, character: 39 }, end: { line: 0, character: 43 } }],
  });
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 3 },
      contentChanges: [
        {
          text: [
            "async function save() { throw new RangeError(); }",
            "save();",
            "try { save(); } catch {}",
            "try { await save(); } catch {}",
            "save().catch(() => {});",
          ].join("\n"),
        },
      ],
    },
  });
  expect((await diagnostic(uri, 3)).params).toMatchObject({
    diagnostics: [
      {
        severity: 1,
        range: { start: { line: 1 } },
        message: expect.stringContaining("RangeError"),
      },
      {
        severity: 1,
        range: { start: { line: 2 } },
        message: expect.stringContaining("RangeError"),
      },
    ],
  });
  const safe = "async function save() {} save();";
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 4 },
      contentChanges: [{ text: safe }],
    },
  });
  expect((await diagnostic(uri, 4)).params).toMatchObject({ diagnostics: [] });
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 3 },
      contentChanges: [{ text }],
    },
  });
  send({
    id: 5,
    method: "textDocument/hover",
    params: {
      textDocument: { uri },
      position: { line: 0, character: safe.lastIndexOf("save()") },
    },
  });
  expect((await receive((message) => message.id === 5)).result).toMatchObject({
    contents: { value: expect.stringContaining("No known thrown errors") },
  });

  // A hover issued while typing must not block later edits or its own
  // cancellation. Only the newest buffer should be published, without any
  // didOpen/didClose (tab switch) being needed to complete the hover.
  const rotation = (throws: string) =>
    [
      "export function hash(value: number) { return rotate32(value, 13); }",
      "function rotate32(value: number, shift: number): number {",
      `  ${throws}`,
      "  return ((value << (shift % 32)) | (value >>> (32 - (shift % 32)))) >>> 0;",
      "}",
    ].join("\n");
  for (let version = 5; version <= 24; version++) {
    send({
      method: "textDocument/didChange",
      params: {
        textDocument: { uri, version },
        contentChanges: [
          {
            text: rotation(
              version === 24
                ? 'if (value === 3) throw new Error("Wut");'
                : 'throw new TypeError("intermediate");',
            ),
          },
        ],
      },
    });
    if (version === 5) {
      send({
        id: 10,
        method: "textDocument/hover",
        params: {
          textDocument: { uri },
          position: { line: 0, character: 45 },
        },
      });
      send({ method: "$/cancelRequest", params: { id: 10 } });
    }
  }
  send({
    method: "workspace/didChangeWatchedFiles",
    params: {
      changes: [{ uri, type: 2 }],
    },
  });
  send({
    id: 11,
    method: "textDocument/hover",
    params: {
      textDocument: { uri },
      position: { line: 0, character: 45 },
    },
  });
  expect((await receive((message) => message.id === 11)).result).toMatchObject({
    contents: { value: "**ErrorScript — may throw / reject**\n\n- `Error`" },
  });
  expect((await receive((message) => message.id === 10)).error?.code).toBe(-32800);
  const versions = messages
    .filter(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        (message.params as { uri: string }).uri === uri &&
        (message.params as { version: number }).version >= 5,
    )
    .map((message) => (message.params as { version: number }).version);
  expect(versions).toEqual([24]);
  await diagnostic(uri, 24);
  await writeFile(
    join(directory, "errconfig.toml"),
    'files = ["*.ts"]\n[checks]\ncast_catch = true',
  );
  send({
    method: "workspace/didChangeWatchedFiles",
    params: {
      changes: [{ uri: pathToFileURL(join(directory, "errconfig.toml")).href, type: 2 }],
    },
  });
  const catchText = "try { throw new Error(); } catch (err_) { const err = err_ as TypeError; }";
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 25 },
      contentChanges: [{ text: catchText }],
    },
  });
  expect((await diagnostic(uri, 25)).params).toMatchObject({
    diagnostics: [
      {
        code: "cast-catch",
        severity: 1,
        source: "ErrorScript",
        range: { start: { line: 0, character: 27 }, end: { line: 0, character: 41 } },
        message: expect.stringContaining("const err = err_ as Error;"),
      },
    ],
  });
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 26 },
      contentChanges: [{ text: catchText.replace("as TypeError", "as Error") }],
    },
  });
  expect((await diagnostic(uri, 26)).params).toMatchObject({ diagnostics: [] });
  send({ method: "textDocument/didClose", params: { textDocument: { uri } } });
  expect((await diagnostic()).params).toMatchObject({ uri, diagnostics: [] });
  send({ id: 3, method: "shutdown" });
  expect((await receive((message) => message.id === 3)).result).toBeNull();
  const exited = once(child, "exit");
  send({ method: "exit" });
  expect(await exited, stderr).toEqual([0, null]);
}, 15000);
