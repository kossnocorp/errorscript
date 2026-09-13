import { fileURLToPath } from "node:url";
import {
  createConnection,
  TextDocuments,
  TextDocumentSyncKind,
} from "vscode-languageserver/node.js";
import { TextDocument } from "vscode-languageserver-textdocument";
import { getEditorDiagnostics, getEditorHover, withEditorProject } from "./editor-project.js";

const connection = createConnection(process.stdin, process.stdout);
const documents = new TextDocuments(TextDocument);
let queue = Promise.resolve();
/** @type {ReturnType<typeof setTimeout> | undefined} */
let timer;
let shuttingDown = false;

/** @template T @param {() => Promise<T>} action */
function enqueue(action) {
  const result = queue.then(action);
  queue = result.then(
    () => {},
    () => {},
  );
  return result;
}

function buffers() {
  return new Map(
    documents.all().map((document) => [fileURLToPath(document.uri), document.getText()]),
  );
}

function scheduleDiagnostics() {
  clearTimeout(timer);
  timer = setTimeout(() => {
    if (shuttingDown) return;
    void enqueue(async () => {
      for (const document of documents.all()) {
        const { uri, version } = document;
        try {
          const diagnostics = await getEditorDiagnostics(fileURLToPath(uri), buffers());
          if (documents.get(uri)?.version === version) {
            connection.sendDiagnostics({ uri, version, diagnostics });
          }
        } catch (error) {
          connection.console.error(String(error));
          if (documents.get(uri)?.version === version) {
            connection.sendDiagnostics({
              uri,
              version,
              diagnostics: [
                {
                  range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
                  severity: 1,
                  source: "errorscript-ts7",
                  message: String(error),
                },
              ],
            });
          }
        }
      }
    });
  }, 120);
}

connection.onInitialize(() => ({
  capabilities: { textDocumentSync: TextDocumentSyncKind.Full, hoverProvider: true },
  serverInfo: { name: "ErrorScript TypeScript 7 PoC", version: "0.0.0" },
}));
documents.onDidChangeContent(scheduleDiagnostics);
documents.onDidClose(({ document }) => {
  connection.sendDiagnostics({ uri: document.uri, diagnostics: [] });
  scheduleDiagnostics();
});
connection.onDidChangeWatchedFiles(scheduleDiagnostics);
connection.onHover((params) =>
  enqueue(() => getEditorHover(fileURLToPath(params.textDocument.uri), buffers(), params.position)),
);
connection.onRequest(
  "errorscript/virtualDocument",
  /** @param {{ uri: string }} params */ (params) =>
    enqueue(() => {
      const fileName = fileURLToPath(params.uri);
      return withEditorProject(
        fileName,
        buffers(),
        async ({ files }) => files.get(fileName)?.text ?? null,
      );
    }),
);
connection.onShutdown(() => {
  shuttingDown = true;
  clearTimeout(timer);
  return queue;
});
connection.onExit(() => process.exit(shuttingDown ? 0 : 1));
documents.listen(connection);
connection.listen();
