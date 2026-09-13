import {
  createMessageConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from "vscode-jsonrpc/node.js";
import { transform } from "./transform.js";

// stdout is reserved for Content-Length-framed JSON-RPC messages.
const connection = createMessageConnection(
  new StreamMessageReader(process.stdin),
  new StreamMessageWriter(process.stdout),
);

connection.onRequest("initialize", () => ({
  protocolVersion: 1,
  positionEncoding: "utf-16",
  diagnosticSource: "errorscript",
}));
// This PoC has no project-specific state or configuration.
connection.onRequest("openProject", () => ({}));
connection.onRequest("closeProject", () => ({}));
connection.onRequest(
  "transform",
  /** @param {{ content: string }} params */ (params) => transform(params.content),
);
connection.listen();
