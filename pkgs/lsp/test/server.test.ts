import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

interface Message {
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
}

test("native stdio server lifecycle, hover, and diagnostics", async ({ onTestFinished }) => {
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
  const diagnostic = () =>
    receive((message) => message.method === "textDocument/publishDiagnostics");
  const uri = "file:///dummy.ts";
  send({
    id: 1,
    method: "initialize",
    params: { processId: process.pid, rootUri: null, capabilities: {} },
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
        text: "// 😀 errorscript-dummy\r\n",
      },
    },
  });
  expect((await diagnostic()).params).toMatchObject({
    uri,
    version: 1,
    diagnostics: [
      {
        source: "ErrorScript",
        range: {
          start: { line: 0, character: 6 },
          end: { line: 0, character: 23 },
        },
      },
    ],
  });
  send({
    id: 2,
    method: "textDocument/hover",
    params: {
      textDocument: { uri },
      position: { line: 0, character: 6 },
    },
  });
  expect((await receive((message) => message.id === 2)).result).toMatchObject({
    contents: { value: expect.stringContaining("ErrorScript") },
  });
  send({
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: 2 },
      contentChanges: [{ text: "const value = 1;" }],
    },
  });
  expect((await diagnostic()).params).toEqual({ uri, version: 2, diagnostics: [] });
  send({ method: "textDocument/didClose", params: { textDocument: { uri } } });
  expect((await diagnostic()).params).toMatchObject({ uri, diagnostics: [] });
  send({ id: 3, method: "shutdown" });
  expect((await receive((message) => message.id === 3)).result).toBeNull();
  const exited = once(child, "exit");
  send({ method: "exit" });
  expect(await exited, stderr).toEqual([0, null]);
}, 15000);
