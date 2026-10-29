// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown } from "../../support/errors";
import { ChildProcess, spawn } from "node:child_process";
import { once } from "node:events";
import { describe, expect, it } from "vitest";

describe("node:child_process", () => {
  describe("ChildProcess", () => {
    describe("[Symbol.dispose]", () => {
      const dispose = ChildProcess.prototype[Symbol.dispose];

      const thrownBy = captureThrown;

      // Only the public killed property changes; the native kill method/handle
      // remain untouched. Restore before waiting for asynchronous exit delivery.
      async function withChild(action: (child: ChildProcess) => void) {
        const child = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {
          stdio: "ignore",
        });
        const closed = new Promise<void>((resolve) => child.once("close", () => resolve()));
        const killed = Object.getOwnPropertyDescriptor(child, "killed")!;
        try {
          await once(child, "spawn");
          action(child);
        } finally {
          Object.defineProperty(child, "killed", killed);
          if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
          await closed;
        }
      }

      describe("invalid receiver or noncallable kill", () => {
        it("throws TypeError", { tags: ["node-only"] }, () => {
          for (const receiver of [undefined, null, 1, {}, { killed: false, kill: 1 }]) {
            const error = thrownBy(() => Reflect.apply(dispose, receiver, []));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("revoked proxy and proxy invariants", () => {
        it("throws TypeError", { tags: ["node-only"] }, () => {
          const revoked = Proxy.revocable({}, {});
          revoked.revoke();
          const target = Object.defineProperty({}, "killed", { value: false });
          const inconsistent = new Proxy(target, { get: () => true });
          for (const receiver of [revoked.proxy, inconsistent]) {
            expect(thrownBy(() => Reflect.apply(dispose, receiver, []))?.constructor).toBe(
              TypeError,
            );
          }
        });
      });

      describe("successful native kill cannot write killed", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          await withChild((child) => {
            Object.defineProperty(child, "killed", { writable: false });
            expect(thrownBy(() => child[Symbol.dispose]())).toHaveExactConstructor(TypeError);
          });
        });
      });

      describe("receiver getter propagation", () => {
        it("throws Error", { tags: ["node-only"] }, () => {
          const supplied = new Error("killed getter");
          const receiver = {
            get killed() {
              throw supplied;
            },
          };

          expect(thrownBy(() => Reflect.apply(dispose, receiver, []))).toBe(supplied);
        });
      });

      describe("successful native kill invokes supplied setter", () => {
        it("throws SyntaxError", { tags: ["node-only"] }, async () => {
          const supplied = new SyntaxError("killed setter");
          await withChild((child) => {
            Object.defineProperty(child, "killed", {
              configurable: true,
              get: () => false,
              set: () => {
                throw supplied;
              },
            });

            expect(thrownBy(() => child[Symbol.dispose]())).toBe(supplied);
          });
        });
      });

      it("throws non-Error value", { tags: ["node-only"] }, () => {
        const supplied = { marker: "receiver effect" };
        const receivers = [
          new Proxy(
            {},
            {
              get() {
                throw supplied;
              },
            },
          ),
          {
            killed: false,
            get kill() {
              throw supplied;
            },
          },
          {
            killed: false,
            kill() {
              throw supplied;
            },
          },
        ];
        for (const receiver of receivers) {
          expect(thrownBy(() => Reflect.apply(dispose, receiver, []))).toBe(supplied);
        }
      });

      it(
        "returns undefined without coercion and on repeated disposal",
        { tags: ["node-only"] },
        async () => {
          const receiver = {
            killed: {
              [Symbol.toPrimitive]() {
                throw new Error("must not coerce");
              },
            },
            get kill() {
              throw new Error("must not read kill");
            },
          };
          expect(Reflect.apply(dispose, receiver, [])).toBeUndefined();
          await withChild((child) => {
            expect(child[Symbol.dispose]()).toBeUndefined();
            expect(child.killed).toBe(true);
            expect(child[Symbol.dispose]()).toBeUndefined();
          });
          const exited = spawn(process.execPath, ["-e", ""], { stdio: "ignore" });
          try {
            await once(exited, "close");
            expect(exited[Symbol.dispose]()).toBeUndefined();
          } finally {
            if (exited.exitCode === null && exited.signalCode === null) {
              const closed = once(exited, "close");
              exited.kill("SIGKILL");
              await closed;
            }
          }
        },
      );

      it("TODO", () => {
        // Native kill failure: needs a deterministic OS-denial fixture, including
        // unhandled Error, handled error event, and throwing-listener paths.
        // Never-spawned native handles also need isolated lifecycle validation.
      });
    });

    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("disconnect", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("kill", () => {
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

    describe("send", () => {
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

  describe("Control", () => {
    describe("ref", () => {
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

  describe("exec", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("execFile", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("execFileSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("execSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("fork", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("spawn", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("spawnSync", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
