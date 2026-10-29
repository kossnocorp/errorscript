// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, throwOnGet } from "../../support/errors";
import { describe, expect, it } from "vitest";

describe("node:http", () => {
  describe("Agent", () => {
    describe("createConnection", () => {
      // Observe real sockets solely to close ones otherwise lost to a synchronous
      // exception. No implementation or callback is replaced by this observer.
      async function fixture() {
        const { Agent } = await import("node:http");
        const { channel } = await import("node:diagnostics_channel");
        const agent = new Agent();
        const sockets = new Set<import("node:net").Socket>();
        const diagnostics = channel("net.client.socket");
        const observer = (message: unknown) => {
          sockets.add((message as { socket: import("node:net").Socket }).socket);
        };
        diagnostics.subscribe(observer);
        return {
          agent,
          call(options: unknown) {
            return agent.createConnection(options as import("node:http").ClientRequestArgs);
          },
          cleanup() {
            diagnostics.unsubscribe(observer);
            for (const socket of sockets) socket.destroy();
            agent.destroy();
          },
        };
      }

      const caught = captureThrown;

      const validations = [
        ["missing endpoint", {}, TypeError],
        ["object mode", { objectMode: true }, TypeError],
        ["high water mark", { highWaterMark: -1 }, TypeError],
        ["encoding", { defaultEncoding: "not-an-encoding" }, TypeError],
        ["descriptor range", { fd: -1 }, RangeError],
        ["keepalive delay", { keepAliveInitialDelay: "bad" }, TypeError],
        ["type of service", { typeOfService: 256 }, RangeError],
        ["timeout", { timeout: -1 }, RangeError],
        ["port range", { port: 65536 }, RangeError],
        ["port type", { port: null }, TypeError],
        ["path type", { path: true }, TypeError],
        ["host null byte", { port: 80, host: "a\0b" }, TypeError],
        ["local address", { port: 80, localAddress: "invalid" }, TypeError],
        ["auto family", { port: 80, autoSelectFamily: 1 }, TypeError],
        ["family timeout", { port: 80, autoSelectFamilyAttemptTimeout: 0 }, RangeError],
        ["lookup type", { port: 80, lookup: true }, TypeError],
        ["DNS family", { port: 80, family: 7 }, TypeError],
        ["block list", { blockList: {} }, TypeError],
        ["abort signal", { signal: true }, TypeError],
      ] as const;

      for (const [branch, options, constructor] of validations) {
        describe(branch, () => {
          const repro = async () => {
            const f = await fixture();
            try {
              const error = caught(() => f.call(options));
              expect(error).toHaveExactConstructor(constructor);
            } finally {
              f.cleanup();
            }
          };
          if (constructor === RangeError) {
            it("throws RangeError", { tags: ["node-only"] }, repro);
          } else {
            it("throws TypeError", { tags: ["node-only"] }, repro);
          }
        });
      }

      describe("receiver and template conversion", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const f = await fixture();
          try {
            for (const call of [
              () => f.agent.createConnection.call(null, {}),
              () => f.call({ host: Symbol("host"), port: 80 }),
            ]) {
              expect((caught(call) as Error)).toHaveExactConstructor(TypeError);
            }
          } finally {
            f.cleanup();
          }
        });
      });

      describe("proxy bypass hostname", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { Agent } = await import("node:http");
          const agent = new Agent({ proxyEnv: { http_proxy: "http://127.0.0.1:80", no_proxy: "localhost" } });
          try {
            const error = caught(() => agent.createConnection({ host: 1 as unknown as string, port: 80 }));
            expect(error).toHaveExactConstructor(TypeError);
          } finally {
            agent.destroy();
          }
        });
      });

      describe("uninitialized namespace diagnostic", () => {
        it("throws ReferenceError", { tags: ["node-only"] }, async () => {
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const { execFile } = await import("node:child_process");
          const { promisify } = await import("node:util");
          const dir = await mkdtemp(join(tmpdir(), "errs-agent-"));
          try {
            const path = join(dir, "namespace.mjs");
            await writeFile(path, `
              import * as ns from './namespace.mjs';
              import { Agent } from 'node:http';
              import assert from 'node:assert/strict';
              const agent = new Agent();
              export let result;
              try { agent.createConnection({ objectMode: ns }); }
              catch (error) { result = error; }
              finally { agent.destroy(); }
              export let constructor;
              assert.equal(result.constructor, ReferenceError);
              console.log('native namespace ReferenceError verified');
            `);
            // Native ESM is essential: Vitest transforms module namespaces.
            const { stdout } = await promisify(execFile)(process.execPath, [path], { timeout: 10_000 });
            expect(stdout.trim()).toBe("native namespace ReferenceError verified");
          } finally {
            await rm(dir, { recursive: true, force: true });
          }
        });
      });

      describe("namespace diagnostic assertion", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const namespace = await import(/* @vite-ignore */ "data:text/javascript," + encodeURIComponent(
            `export const value = { [Symbol.for('nodejs.util.inspect.custom')]() { throw 42; } };`,
          ));
          const f = await fixture();
          try {
            const error = caught(() => f.call({ objectMode: namespace }));
            expect(error).toHaveExactConstructor(Error);
          } finally {
            f.cleanup();
          }
        });
      });

      describe("closed bound socket adoption", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const { BoundSocket } = await import("node:net");
          const bound = new BoundSocket({ host: "127.0.0.1", port: 0 });
          const f = await fixture();
          try {
            bound.close();
            const error = caught(() => f.call({ handle: bound, host: "127.0.0.1", port: 80 }));
            expect(error).toHaveExactConstructor(Error);
          } finally {
            bound[Symbol.dispose]();
            f.cleanup();
          }
        });
      });

      describe("custom inspector propagation", () => {
        it("throws SyntaxError", { tags: ["node-only"] }, async () => {
          const { inspect } = await import("node:util");
          const sentinel = new SyntaxError("supplied inspector failure");
          const f = await fixture();
          try {
            const value = { [inspect.custom]() { throw sentinel; } };
            const error = caught(() => f.call({ objectMode: value }));
            expect(error).toBe(sentinel);
          } finally {
            f.cleanup();
          }
        });
      });

      it("throws non-Error value", { tags: ["node-only"] }, async () => {
        const sentinel = { supplied: true };
        const f = await fixture();
        try {
          const options = [
            { get host() { throw sentinel; } },
            throwOnGet({}, "objectMode", sentinel),
            new Proxy({}, { ownKeys() { throw sentinel; } }),
            { host: { [Symbol.toPrimitive]() { throw sentinel; } } },
            { port: 80, lookup() { throw sentinel; } },
            { keepAliveInitialDelay: { get constructor() { throw sentinel; } } },
            { port: 80, onread: { buffer() { throw sentinel; }, callback() {} } },
          ];
          for (const option of options) expect(caught(() => f.call(option))).toBe(sentinel);
        } finally {
          f.cleanup();
        }
      });

      describe("blocked address error event", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const { BlockList } = await import("node:net");
          const blockList = new BlockList();
          blockList.addAddress("127.0.0.1");
          const f = await fixture();
          try {
            const socket = f.call({ host: "127.0.0.1", port: 80, blockList })!;
            const error = await new Promise<Error>((resolve) => socket.once("error", resolve));
            expect(error).toHaveExactConstructor(Error);
          } finally {
            f.cleanup();
          }
        });
      });

      describe("multiple blocked addresses error event", () => {
        it("throws AggregateError", { tags: ["node-only"] }, async () => {
          const { BlockList } = await import("node:net");
          const blockList = new BlockList();
          blockList.addAddress("127.0.0.1");
          blockList.addAddress("::1", "ipv6");
          const f = await fixture();
          try {
            const socket = f.call({ host: "localhost", port: 80, blockList, autoSelectFamily: true,
              lookup(_host: string, _options: unknown, callback: Function) {
                callback(null, [{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }]);
              },
            })!;
            const error = await new Promise<AggregateError>((resolve) => socket.once("error", resolve));
            expect(error).toHaveExactConstructor(AggregateError);
            expect(error.errors).toHaveLength(2);
            for (const inner of error.errors) {
              expect(inner).toHaveExactConstructor(Error);
            }
          } finally {
            f.cleanup();
          }
        });
      });

      describe("lookup error event propagation", () => {
        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          const sentinel = { supplied: true };
          const f = await fixture();
          try {
            const socket = f.call({ host: "localhost", port: 80,
              lookup(_host: string, _options: unknown, callback: Function) { callback(sentinel); },
            })!;
            const error = await new Promise<unknown>((resolve) => socket.once("error", resolve));
            expect(error).toBe(sentinel);
          } finally {
            f.cleanup();
          }
        });
      });

      for (const [branch, address, family, constructor] of [
        ["invalid lookup address event", "invalid", 4, TypeError],
        ["invalid lookup family event", "127.0.0.1", 7, RangeError],
      ] as const) {
        describe(branch, () => {
          const repro = async () => {
            const f = await fixture();
            try {
              const socket = f.call({ host: "localhost", port: 80, autoSelectFamily: false,
                lookup(_host: string, _options: unknown, callback: Function) { callback(null, address, family); },
              })!;
              const error = await new Promise<Error>((resolve) => socket.once("error", resolve));
              expect(error).toHaveExactConstructor(constructor);
            } finally {
              f.cleanup();
            }
          };
          if (constructor === RangeError) {
            it("throws RangeError", { tags: ["node-only"] }, repro);
          } else {
            it("throws TypeError", { tags: ["node-only"] }, repro);
          }
        });
      }

      it("TODO", () => {
        // Pending: HTTPS-proxy TLS/native paths and remaining host-specific paths.
      });
    });

    describe("destroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("keepSocketAlive", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("reuseSocket", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ClientRequest", () => {
    describe("abort", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getRawHeaderNames", () => {
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

    describe("onSocket", () => {
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

    describe("setNoDelay", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setSocketKeepAlive", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ClientRequestArgs", () => {
    describe("_defaultAgent", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("agent", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("createConnection", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("IncomingMessage", () => {
    describe("destroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("OutgoingMessage", () => {
    describe("addTrailers", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("appendHeader", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("flushHeaders", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getHeader", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getHeaderNames", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getHeaders", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("hasHeader", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("removeHeader", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setHeader", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setHeaders", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("RequestListener", () => {
    it("TODO", () => {
      // TODO
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

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ServerOptions", () => {
    describe("shouldUpgradeCallback", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ServerResponse", () => {
    describe("assignSocket", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("detachSocket", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeContinue", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeEarlyHints", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeHead", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeInformation", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeProcessing", () => {
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

    describe("destroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("keepSocketAlive", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("reuseSocket", () => {
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

  describe("setGlobalProxyFromEnv", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setMaxIdleHTTPParsers", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("validateHeaderName", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("validateHeaderValue", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
