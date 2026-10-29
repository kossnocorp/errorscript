// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import "../../support/errors";

describe("node:dgram", () => {
  describe("Socket", () => {
    describe("[Symbol.asyncDispose]", () => {
      describe("registration array limit", () => {
        it("throws RangeError", async () => {
          const { createSocket, Socket } = await import("node:dgram");
          const socket = createSocket("udp4");
          const events = { close: new Array(2 ** 32 - 1) };
          const receiver = new Proxy(socket, {
            get(target, key, receiver) {
              return key === "_events" ? events : Reflect.get(target, key, receiver);
            },
          });
          try {
            const result = Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []);
            await expect(result).rejects.toHaveExactConstructor(RangeError);
          } finally {
            await socket[Symbol.asyncDispose]();
          }
        });
      });

      describe("invalid-close diagnostic and warning inspection", () => {
        async function checkInspection(
          value: object,
          check: (result: Promise<void>) => Promise<void>,
        ) {
          const { createSocket, Socket } = await import("node:dgram");
          const { inspect } = await import("node:util");
          for (const mode of ["diagnostic", "warning"]) {
            const stackLimit = Object.getOwnPropertyDescriptor(Error, "stackTraceLimit");
            const socket = createSocket("udp4");
            const receiver = mode === "diagnostic" ? new Proxy(socket, {
              get(target, key, receiver) {
                return key === "close" ? value : Reflect.get(target, key, receiver);
              },
            }) : socket;
            try {
              if (mode === "warning") {
                socket.setMaxListeners(1);
                socket.on("close", () => {});
                Object.defineProperty(socket, inspect.custom, { value: () => value, configurable: true });
              }
              await check(Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []));
              if (mode === "warning") expect(socket.listenerCount("close")).toBe(2);
            } finally {
              // Failed validator diagnostics can leave Node's temporary limit at zero.
              if (stackLimit) Object.defineProperty(Error, "stackTraceLimit", stackLimit);
              else Reflect.deleteProperty(Error, "stackTraceLimit");
              Reflect.deleteProperty(socket, inspect.custom);
              socket.removeAllListeners("close");
              await socket[Symbol.asyncDispose]();
            }
          }
        }

        it("throws RangeError", async () => {
          const { inspect } = await import("node:util");
          const first = Object.create(null);
          const second = Object.create(null);
          first[inspect.custom] = () => second;
          second[inspect.custom] = () => first;
          // The hooks only return values; recursion happens inside native inspect.
          await checkInspection(first, async (result) => {
            await expect(result).rejects.toHaveExactConstructor(RangeError);
          });
        });

        it("throws TypeError", async () => {
          const value = new Uint8Array(1);
          const buffer = value.buffer;
          Object.setPrototypeOf(value, null);
          buffer.transfer();
          await checkInspection(value, async (result) => {
            await expect(result).rejects.toHaveExactConstructor(TypeError);
          });
        });

        it("throws non-Error value", async () => {
          const { inspect } = await import("node:util");
          const supplied = { sentinel: "inspection hook" };
          const value = Object.create(null, {
            [inspect.custom]: { value: () => { throw supplied; } },
          });
          await checkInspection(value, async (result) => {
            await expect(result).rejects.toBe(supplied);
          });
        });
      });

      describe("native namespace diagnostic and warning inspection", () => {
        it("throws ReferenceError", async () => {
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const { spawnSync } = await import("node:child_process");
          const directory = await mkdtemp(join(tmpdir(), "errs-dgram-dispose-"));
          try {
            const file = join(directory, "namespace.mjs");
            await writeFile(file, `
              import * as namespace from './namespace.mjs';
              import { createSocket, Socket } from 'node:dgram';
              import { inspect } from 'node:util';
              import assert from 'node:assert/strict';
              for (const mode of ['diagnostic', 'warning']) {
                const socket = createSocket('udp4');
                const receiver = mode === 'diagnostic' ? new Proxy(socket, {
                  get(target, key, receiver) {
                    return key === 'close' ? namespace : Reflect.get(target, key, receiver);
                  }
                }) : socket;
                try {
                  if (mode === 'warning') {
                    socket.setMaxListeners(1);
                    socket.on('close', () => {});
                    socket[inspect.custom] = () => namespace;
                  }
                  await assert.rejects(
                    Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []),
                    error => error.constructor === ReferenceError
                  );
                } finally {
                  delete socket[inspect.custom];
                  socket.removeAllListeners('close');
                  await socket[Symbol.asyncDispose]();
                }
              }
              // The namespace export stays in TDZ until both callable invocations finish.
              export let constructor;
              console.log('verified both ReferenceError rejections');
            `);
            const child = spawnSync(process.execPath, [file], { encoding: "utf8", timeout: 5000 });
            expect(child.error).toBeUndefined();
            expect(child.status).toBe(0);
            expect(child.stderr).toBe("");
            expect(child.stdout.trim()).toBe("verified both ReferenceError rejections");
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });

        it("throws Error", async () => {
          const { spawnSync } = await import("node:child_process");
          const child = spawnSync(process.execPath, ["--input-type=module", "-e", `
            import { createSocket, Socket } from 'node:dgram';
            import { inspect } from 'node:util';
            import assert from 'node:assert/strict';
            const namespace = await import('data:text/javascript,' + encodeURIComponent(
              'export const payload = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };'
            ));
            const fn = function() {};
            Object.defineProperty(fn, 'name', { value: namespace });
            const value = Object.create(null, {
              [inspect.custom]: { value: () => fn }
            });
            for (const mode of ['diagnostic', 'warning']) {
              const socket = createSocket('udp4');
              const receiver = mode === 'diagnostic' ? new Proxy(socket, {
                get(target, key, receiver) {
                  return key === 'close' ? value : Reflect.get(target, key, receiver);
                }
              }) : socket;
              try {
                if (mode === 'warning') {
                  socket.setMaxListeners(1);
                  socket.on('close', () => {});
                  socket[inspect.custom] = () => value;
                }
                await assert.rejects(
                  Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []),
                  error => error !== value && error.constructor === Error
                );
              } finally {
                delete socket[inspect.custom];
                socket.removeAllListeners('close');
                await socket[Symbol.asyncDispose]();
              }
            }
            console.log('verified both internal Error rejections');
          `], { encoding: "utf8", timeout: 5000 });
          expect(child.error).toBeUndefined();
          expect(child.status).toBe(0);
          expect(child.stderr).toBe("");
          expect(child.stdout.trim()).toBe("verified both internal Error rejections");
        });
      });

      it("throws TypeError", async () => {
        const { Socket } = await import("node:dgram");
        const revoked = Proxy.revocable({}, {});
        revoked.revoke();
        for (const receiver of [undefined, null, 1, {}, Object.create(Socket.prototype), revoked.proxy]) {
          const result = Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []);
          await expect(result).rejects.toHaveExactConstructor(TypeError);
        }
      });

      describe("non-callable close on a supplied receiver", () => {
        it("throws TypeError", async () => {
          const { createSocket, Socket } = await import("node:dgram");
          const socket = createSocket("udp4");
          const receiver = new Proxy(socket, {
            get(target, key, receiver) {
              return key === "close" ? 0 : Reflect.get(target, key, receiver);
            },
          });
          try {
            const result = Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, []);
            await expect(result).rejects.toHaveExactConstructor(TypeError);
          } finally {
            await socket[Symbol.asyncDispose]();
          }
        });
      });

      it("throws Error", async () => {
        const { createSocket } = await import("node:dgram");
        const { once } = await import("node:events");
        const socket = createSocket("udp4");
        const closed = once(socket, "close");
        const reenter = (event: string | symbol) => {
          if (event === "close") {
            socket.removeListener("newListener", reenter);
            socket.close();
          }
        };
        socket.on("newListener", reenter);
        try {
          const result = socket[Symbol.asyncDispose]();
          await expect(result).rejects.toHaveExactConstructor(Error);
        } finally {
          socket.removeListener("newListener", reenter);
          await socket[Symbol.asyncDispose]();
          await closed;
        }
      });

      describe("captureRejections then getter fails without an error listener", () => {
        it("throws Error", async () => {
          const { createSocket } = await import("node:dgram");
          const { EventEmitter } = await import("node:events");
          const previous = EventEmitter.captureRejections;
          const socket = (() => {
            try {
              EventEmitter.captureRejections = true;
              return createSocket("udp4");
            } finally {
              EventEmitter.captureRejections = previous;
            }
          })();
          const supplied = { sentinel: "then getter" };
          const listener = (event: string | symbol) => {
            if (event === "close") return { get then() { throw supplied; } };
          };
          socket.on("newListener", listener);
          try {
            const result = socket[Symbol.asyncDispose]();
            await expect(result).rejects.toHaveExactConstructor(Error);
            await expect(result).rejects.toSatisfy((error: { context: unknown }) => error.context === supplied);
          } finally {
            socket.removeListener("newListener", listener);
            await socket[Symbol.asyncDispose]();
          }
        });
      });

      it("throws SyntaxError", async () => {
        const { createSocket } = await import("node:dgram");
        const socket = createSocket("udp4");
        const supplied = new SyntaxError("newListener sentinel");
        const listener = (event: string | symbol) => {
          if (event === "close") throw supplied;
        };
        socket.on("newListener", listener);
        try {
          const result = socket[Symbol.asyncDispose]();
          await expect(result).rejects.toBe(supplied);
          await expect(result).rejects.toHaveExactConstructor(SyntaxError);
        } finally {
          socket.removeListener("newListener", listener);
          await socket[Symbol.asyncDispose]();
        }
      });

      it("throws non-Error value", async () => {
        const { createSocket, Socket } = await import("node:dgram");
        const socket = createSocket("udp4");
        const supplied = { sentinel: "receiver get trap" };
        try {
          for (const failingKey of ["state", "close"]) {
            const receiver = new Proxy(socket, {
              get(target, key, receiver) {
                if (failingKey === "state" || key === "close") throw supplied;
                return Reflect.get(target, key, receiver);
              },
            });
            await expect(Reflect.apply(Socket.prototype[Symbol.asyncDispose], receiver, [])).rejects.toBe(supplied);
          }
        } finally {
          await socket[Symbol.asyncDispose]();
        }
      });

      describe("close-listener exception is event delivery, not rejection", () => {
        it("throws non-Error value", async () => {
          const { spawnSync } = await import("node:child_process");
          // Isolate the deliberate uncaught event exception from the test runner.
          const child = spawnSync(process.execPath, ["--input-type=module", "-e", `
            import { createSocket } from 'node:dgram';
            const socket = createSocket('udp4');
            const supplied = { sentinel: 'close event' };
            let settled = false;
            process.once('uncaughtException', (error) => {
              console.log(JSON.stringify({ same: error === supplied, settled }));
            });
            socket.on('close', () => { throw supplied; });
            socket[Symbol.asyncDispose]().then(
              () => { settled = true; }, () => { settled = true; }
            );
          `], { encoding: "utf8", timeout: 5000 });
          expect(child.error).toBeUndefined();
          expect(child.status).toBe(0);
          expect(child.stderr).toBe("");
          expect(JSON.parse(child.stdout)).toEqual({ same: true, settled: false });
        });
      });

      describe("duplicate queued disposal fails during listening delivery", () => {
        it("throws Error", async () => {
          const { spawnSync } = await import("node:child_process");
          const child = spawnSync(process.execPath, ["--input-type=module", "-e", `
            import { createSocket } from 'node:dgram';
            const socket = createSocket('udp4');
            let failure;
            process.once('uncaughtException', (error) => {
              failure = { exact: error.constructor === Error };
            });
            socket.connect(9, '127.0.0.1');
            await Promise.all([socket[Symbol.asyncDispose](), socket[Symbol.asyncDispose]()]);
            console.log(JSON.stringify({ failure, fulfilled: true }));
          `], { encoding: "utf8", timeout: 5000 });
          expect(child.error).toBeUndefined();
          expect(child.status).toBe(0);
          expect(child.stderr).toBe("");
          expect(JSON.parse(child.stdout)).toEqual({
            failure: { exact: true },
            fulfilled: true,
          });
        });
      });

      it("fulfills for unbound, bound, queued and already closed sockets", async () => {
        const { createSocket } = await import("node:dgram");
        const { once } = await import("node:events");
        for (const state of ["unbound", "bound", "queued"]) {
          const socket = createSocket("udp4");
          try {
            if (state === "bound") {
              const listening = once(socket, "listening");
              socket.bind(0, "127.0.0.1");
              await listening;
            } else if (state === "queued") {
              // connect performs an asynchronous local bind and enqueues _connect.
              socket.connect(9, "127.0.0.1");
            }
            const closed = once(socket, "close");
            await expect(socket[Symbol.asyncDispose]()).resolves.toBeUndefined();
            await closed;
            await expect(socket[Symbol.asyncDispose]()).resolves.toBeUndefined();
          } finally {
            await socket[Symbol.asyncDispose]();
          }
        }
      });
    });

    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("addMembership", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("addSourceSpecificMembership", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("address", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("bind", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("bindSync", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("close", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("connect", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("connectSync", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("disconnect", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("dropMembership", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("dropSourceSpecificMembership", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getRecvBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getSendBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getSendQueueCount", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getSendQueueSize", () => {
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

    describe("ref", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("remoteAddress", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("send", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setBroadcast", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setMulticastInterface", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setMulticastLoopback", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setMulticastTTL", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setRecvBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setSendBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTTL", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unref", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("SocketOptions", () => {
    describe("lookup", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("createSocket", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
