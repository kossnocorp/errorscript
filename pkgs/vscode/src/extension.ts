import {
  window,
  workspace,
  Range,
  DecorationRangeBehavior,
  type ExtensionContext,
  type TextEditor,
} from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node.js";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const underline = window.createTextEditorDecorationType({
    textDecoration: "underline",
    rangeBehavior: DecorationRangeBehavior.ClosedClosed,
  });
  const calls = new Map<string, { version: number; ranges: Range[] }>();
  const render = (editor: TextEditor) => {
    const result = calls.get(editor.document.uri.toString());
    const enabled = workspace
      .getConfiguration("errorscript", editor.document.uri)
      .get<boolean>("highlightThrowingCalls", true);
    editor.setDecorations(
      underline,
      enabled && result?.version === editor.document.version ? result.ranges : [],
    );
  };
  context.subscriptions.push(
    underline,
    window.onDidChangeVisibleTextEditors((editors) => editors.forEach(render)),
    workspace.onDidChangeTextDocument(({ document }) => {
      calls.delete(document.uri.toString());
      window.visibleTextEditors.filter((editor) => editor.document === document).forEach(render);
    }),
    workspace.onDidCloseTextDocument((document) => calls.delete(document.uri.toString())),
    workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("errorscript.highlightThrowingCalls"))
        window.visibleTextEditors.forEach(render);
    }),
  );
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
  context.subscriptions.push(
    client.onNotification(
      "errorscript/throwingCalls",
      (params: {
        uri: string;
        version: number;
        ranges: {
          start: { line: number; character: number };
          end: { line: number; character: number };
        }[];
      }) => {
        const document = workspace.textDocuments.find(
          (document) => document.uri.toString() === params.uri,
        );
        if (!document || document.version !== params.version) return;
        calls.set(params.uri, {
          version: params.version,
          ranges: params.ranges.map(
            ({ start, end }) => new Range(start.line, start.character, end.line, end.character),
          ),
        });
        window.visibleTextEditors.filter((editor) => editor.document === document).forEach(render);
      },
    ),
  );
  await client.start();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}
