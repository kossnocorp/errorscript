import { workspace, type ExtensionContext } from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node.js";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const watcher = workspace.createFileSystemWatcher(
    "**/*.{ts,tsx,mts,cts,js,jsx,mjs,cjs,json,toml}",
  );
  context.subscriptions.push(watcher);
  client = new LanguageClient(
    "errorscript",
    "ErrorScript",
    {
      module: context.asAbsolutePath("dist/lsp/server.cjs"),
      transport: TransportKind.stdio,
    },
    {
      documentSelector: [
        { language: "javascript" },
        { language: "javascriptreact" },
        { language: "typescript" },
        { language: "typescriptreact" },
      ],
      diagnosticCollectionName: "errorscript",
      synchronize: { fileEvents: watcher },
    },
  );
  context.subscriptions.push(client);
  await client.start();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}
