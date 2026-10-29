// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { captureThrown, throwOnGet } from "../../support/errors";

describe("node:dns", () => {
  describe("Resolver", () => {
    describe("cancel", () => {
      it("throws TypeError", { tags: ["node-only"] }, async () => {
        const { Resolver } = await import("node:dns");
        for (const receiver of [undefined, null, {}, 1, { _handle: null }, { _handle: {} }, { _handle: { cancel: 1 } }]) {
          let caught: unknown;
          try {
            Reflect.apply(Resolver.prototype.cancel, receiver, []);
          } catch (error) {
            caught = error;
          }
          expect(caught).toHaveExactConstructor(TypeError);
        }
      });

      describe("proxy validation", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { Resolver } = await import("node:dns");
          const revoked = Proxy.revocable({}, {});
          revoked.revoke();
          const invariant = new Proxy(Object.defineProperty({}, "_handle", { value: null }), {
            get: () => ({}),
          });
          for (const receiver of [revoked.proxy, invariant]) {
            let caught: unknown;
            try {
              Reflect.apply(Resolver.prototype.cancel, receiver, []);
            } catch (error) {
              caught = error;
            }
            expect(caught).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("native receiver signature", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { spawnSync } = await import("node:child_process");
          // Do not risk an internal binding assertion in the test runner.
          const child = spawnSync(process.execPath, ["--input-type=commonjs", "-e", `
            const assert = require('node:assert/strict');
            const { Resolver } = require('node:dns');
            const resolver = new Resolver();
            try {
              const receiver = { _handle: { cancel: resolver._handle.cancel } };
              assert.throws(() => Resolver.prototype.cancel.call(receiver), (error) => {
                assert.equal(error.constructor, TypeError);
                return true;
              });
              process.stdout.write('native-TypeError');
            } finally {
              resolver.cancel();
            }
          `], { encoding: "utf8", timeout: 5000 });
          expect(child.error).toBeUndefined();
          expect(child.signal).toBeNull();
          expect(child.status, child.stderr).toBe(0);
          expect(child.stdout).toBe("native-TypeError");
        });
      });

      it("throws Error", { tags: ["node-only"] }, async () => {
        const { Resolver } = await import("node:dns");
        const sentinel = new Error("receiver getter");
        const receiver = { get _handle() { throw sentinel; } };
        let caught: unknown;
        try {
          Reflect.apply(Resolver.prototype.cancel, receiver, []);
        } catch (error) {
          caught = error;
        }
        expect(caught).toBe(sentinel);
      });

      it("throws SyntaxError", { tags: ["node-only"] }, async () => {
        const { Resolver } = await import("node:dns");
        const sentinel = new SyntaxError("handle getter");
        const receiver = { _handle: { get cancel() { throw sentinel; } } };
        let caught: unknown;
        try {
          Reflect.apply(Resolver.prototype.cancel, receiver, []);
        } catch (error) {
          caught = error;
        }
        expect(caught).toBe(sentinel);
      });

      it("throws non-Error value", { tags: ["node-only"] }, async () => {
        const { Resolver } = await import("node:dns");
        const sentinel = { supplied: true };
        const receiver = throwOnGet({}, "_handle", sentinel);
        expect(captureThrown(() => Reflect.apply(Resolver.prototype.cancel, receiver, []))).toBe(sentinel);
      });

      describe("callback delivery (not a synchronous throw)", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const { Resolver } = await import("node:dns");
          const { createSocket } = await import("node:dgram");
          const { once } = await import("node:events");
          const socket = createSocket("udp4");
          const resolver = new Resolver();
          const completions: Promise<{ error: NodeJS.ErrnoException | null; returned: boolean }>[] = [];
          let returned = false;
          try {
            const listening = once(socket, "listening");
            socket.bind(0, "127.0.0.1");
            await listening;
            resolver.setServers([`127.0.0.1:${socket.address().port}`]);
            expect(resolver.cancel()).toBeUndefined(); // Idle channel.
            for (const kind of ["resolve4", "reverse"] as const) {
              completions.push(new Promise((resolve) => {
                const callback = (error: NodeJS.ErrnoException | null) => resolve({ error, returned });
                if (kind === "resolve4") resolver.resolve4("cancel.invalid", callback);
                else resolver.reverse("192.0.2.1", callback);
              }));
            }
            expect(resolver.cancel()).toBeUndefined();
            returned = true;
            expect(resolver.cancel()).toBeUndefined(); // Already cancelled.
            const results = await Promise.all(completions);
            for (const { error, returned: afterReturn } of results) {
              expect(afterReturn).toBe(true);
              expect(error).toHaveExactConstructor(Error);
              // Distinguish cancel delivery from an unrelated resolver/network failure.
              expect(error?.code).toBe("ECANCELLED");
            }
            expect(results.map(({ error }) => error?.syscall)).toEqual(["queryA", "getHostByAddr"]);
          } finally {
            resolver.cancel();
            await Promise.all(completions);
            await new Promise<void>((resolve) => socket.close(() => resolve()));
          }
        });

        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          // Isolate uncaught callback delivery from Vitest's own exception handlers.
          const { spawnSync } = await import("node:child_process");
          const child = spawnSync(process.execPath, ["--input-type=commonjs", "-e", `
            const assert = require('node:assert/strict');
            const { Resolver } = require('node:dns');
            const socket = require('node:dgram').createSocket('udp4');
            const resolver = new Resolver();
            const sentinel = { supplied: true };
            let returned = false;
            process.once('uncaughtException', (error) => {
              try {
                assert.equal(error, sentinel);
                assert.equal(returned, true);
                process.stdout.write('identity-after-return');
              } finally {
                resolver.cancel();
                socket.close();
              }
            });
            socket.bind(0, '127.0.0.1', () => {
              resolver.setServers(['127.0.0.1:' + socket.address().port]);
              resolver.resolve4('cancel.invalid', (error) => {
                assert.equal(error.constructor, Error);
                // Prove cancel delivered this error, rather than DNS/network failure.
                assert.equal(error.code, 'ECANCELLED');
                throw sentinel;
              });
              assert.equal(resolver.cancel(), undefined);
              returned = true;
            });
          `], { encoding: "utf8", timeout: 5000 });
          expect(child.error).toBeUndefined();
          expect(child.signal).toBeNull();
          expect(child.status, child.stderr).toBe(0);
          expect(child.stdout).toBe("identity-after-return");
        });
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setLocalAddress", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("getDefaultResultOrder", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getServers", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("lookup", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("lookupService", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolve", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolve4", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolve6", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveAny", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveCaa", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveCname", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveMx", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveNaptr", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveNs", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolvePtr", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveSoa", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveSrv", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveTlsa", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("resolveTxt", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("reverse", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setDefaultResultOrder", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setServers", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
