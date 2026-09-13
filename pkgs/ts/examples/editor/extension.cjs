const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node.js");

/** @type {import("vscode-languageclient/node.js").LanguageClient | undefined} */
let client;

/** @param {import("vscode").ExtensionContext} context */
exports.activate = async function (context) {
  const watcher = vscode.workspace.createFileSystemWatcher("**/*.{ts,json}");
  const languageClient = new LanguageClient(
    "errorscript-ts7",
    "ErrorScript TypeScript 7 PoC",
    { command: "node", args: [require.resolve("@errorscript/ts/src/editor-server.js")] },
    {
      documentSelector: [{ scheme: "file", language: "errorscript-typescript" }],
      diagnosticCollectionName: "errorscript-ts7",
      synchronize: { fileEvents: watcher },
    },
  );
  client = languageClient;
  context.subscriptions.push(watcher, languageClient);
  await languageClient.start();
  context.subscriptions.push(
    vscode.commands.registerCommand("errorscript-ts7.virtualDocument", async () => {
      const document = vscode.window.activeTextEditor?.document;
      if (document?.languageId !== "errorscript-typescript") return;
      /** @type {string | null} */
      const text = await languageClient.sendRequest("errorscript/virtualDocument", {
        uri: document.uri.toString(),
      });
      if (text !== null) {
        const virtual = await vscode.workspace.openTextDocument({
          content: text,
          language: "typescript",
        });
        await vscode.window.showTextDocument(virtual, {
          viewColumn: vscode.ViewColumn.Beside,
          preview: true,
        });
      }
    }),
  );
};

exports.deactivate = async function () {
  await client?.stop();
};
