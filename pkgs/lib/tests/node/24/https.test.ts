// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import "../../support/errors";
import { Agent } from "node:https";
import { runNode } from "../../support/node";
import { createServer, type Socket } from "node:net";

describe("node:https", () => {
  describe("Agent", () => {
    describe("createConnection", () => {
      function failure(options: unknown) {
        const agent = new Agent();
        try {
          const socket = agent.createConnection(options as Parameters<Agent["createConnection"]>[0]);
          socket?.destroy();
        } catch (error) {
          return error;
        } finally {
          agent.destroy();
        }
        throw new Error("Expected createConnection to throw");
      }

      it("throws TypeError", { tags: ["node-only"] }, () => {
        const error = failure({ checkServerIdentity: 0 });
        expect(error).toHaveExactConstructor(TypeError);
      });

      it("throws TypeError", { tags: ["node-only"] }, () => {
        let error: unknown;
        try {
          Reflect.apply(Agent.prototype.createConnection, undefined, [{}]);
        } catch (caught) {
          error = caught;
        }
        expect(error).toHaveExactConstructor(TypeError);
      });

      it("throws TypeError", { tags: ["node-only"] }, () => {
        const error = failure({ minVersion: "not-a-TLS-version" });
        expect(error).toHaveExactConstructor(TypeError);
      });

      it("throws RangeError", { tags: ["node-only"] }, () => {
        const error = failure({ minDHSize: 0 });
        expect(error).toHaveExactConstructor(RangeError);
      });

      it("throws Error", { tags: ["node-only"] }, () => {
        const error = failure({ cert: "not a certificate" });
        expect(error).toHaveExactConstructor(Error);
      });

      it("throws Error", { tags: ["node-only"] }, () => {
        const error = failure({ ciphers: "no-such-cipher" });
        expect(error).toHaveExactConstructor(Error);
      });

      it("throws non-Error value", { tags: ["node-only"] }, () => {
        const sentinel = { origin: "options getter" };
        expect(failure({ get _agentKey() { throw sentinel; } })).toBe(sentinel);
      });

      it("throws non-Error value", { tags: ["node-only"] }, () => {
        const sentinel = Symbol("proxy ownKeys");
        const options = new Proxy({}, { ownKeys() { throw sentinel; } });
        expect(failure(options)).toBe(sentinel);
      });

      it("throws ReferenceError", { tags: ["node-only"] }, () => {
        // A linked but unevaluated real module namespace has an uninitialized
        // export. The read of options.port is inside createConnection, not setup.
        const output = runNode([
          "--experimental-vm-modules", "--input-type=module", "-e", `
          import assert from 'node:assert/strict';
          import { Agent } from 'node:https';
          import { SourceTextModule } from 'node:vm';
          const mod = new SourceTextModule('export let port = 443;');
          await mod.link(() => {});
          const agent = new Agent();
          let error;
          try { agent.createConnection(mod.namespace); }
          catch (caught) { error = caught; }
          finally { agent.destroy(); }
          assert.equal(error?.constructor, ReferenceError);
          console.log('verified ReferenceError');
        `], { env: { ...process.env, NODE_DEBUG: "" } });
        expect(output.trim()).toBe("verified ReferenceError");
      });

      it("throws Error", { tags: ["node-only"] }, async () => {
        // This is error-first callback delivery, NOT a synchronous throw or a
        // rejected promise returned by createConnection.
        const accepted = new Set<Socket>();
        const server = createServer((socket) => {
          accepted.add(socket);
          socket.on("error", () => {});
          socket.once("data", () => socket.write("HTTP/1.1 502 Bad Gateway\r\n\r\n"));
        });
        let agent: Agent | undefined;
        let client: ReturnType<Agent["createConnection"]> = undefined;
        try {
          await new Promise<void>((resolve, reject) => {
            server.once("error", reject);
            server.listen(0, "127.0.0.1", resolve);
          });
          const address = server.address();
          if (!address || typeof address === "string") throw new Error("Expected TCP address");
          agent = new Agent({ proxyEnv: { https_proxy: `http://127.0.0.1:${address.port}` } });
          const error = await new Promise<Error | null>((resolve) => {
            client = agent!.createConnection({ host: "target.invalid", port: 443 }, (err) => resolve(err));
          });
          expect(error).toHaveExactConstructor(Error);
          expect(error).toHaveProperty("statusCode", 502);
        } finally {
          client?.destroy();
          for (const socket of accepted) socket.destroy();
          agent?.destroy();
          if (server.listening) await new Promise<void>((resolve) => server.close(() => resolve()));
        }
      });

      it("throws Error", { tags: ["node-only"] }, async () => {
        // Direct TLS errors arrive on the returned socket, not the optional
        // createConnection callback (which is only used on the proxy branch).
        const accepted = new Set<Socket>();
        const server = createServer((socket) => {
          accepted.add(socket);
          socket.on("error", () => {});
          socket.end();
        });
        const agent = new Agent();
        let client: ReturnType<Agent["createConnection"]> = undefined;
        let callbacks = 0;
        try {
          await new Promise<void>((resolve, reject) => {
            server.once("error", reject);
            server.listen(0, "127.0.0.1", resolve);
          });
          const address = server.address();
          if (!address || typeof address === "string") throw new Error("Expected TCP address");
          const error = await new Promise<Error>((resolve) => {
            client = agent.createConnection({ host: "127.0.0.1", port: address.port }, () => { callbacks++; });
            client!.once("error", resolve);
          });
          expect(error).toHaveExactConstructor(Error);
          expect(callbacks).toBe(0);
        } finally {
          client?.destroy();
          for (const socket of accepted) socket.destroy();
          agent.destroy();
          if (server.listening) await new Promise<void>((resolve) => server.close(() => resolve()));
        }
      });

      it("TODO", () => {
        // Blocked: remaining native TLS, diagnostic/inspect, proxy-success,
        // supplied-socket and deferred callback paths are not exhaustively proved.
      });
    });

    describe("getName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("RequestOptions", () => {
    describe("checkServerIdentity", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Server", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("closeAllConnections", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("closeIdleConnections", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("on", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("once", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("prependListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("prependOnceListener", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("createServer", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("get", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("globalAgent", () => {
    describe("createConnection", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("request", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
