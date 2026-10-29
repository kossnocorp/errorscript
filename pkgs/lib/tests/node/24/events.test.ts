// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { runNode } from "../../support/node";

describe("node:events", () => {
  describe("EventEmitter", () => {
    describe("EventEmitterAsyncResource", () => {
      describe("emitDestroy", () => {
        // Destroy hooks and their process-fatal failures must not touch Vitest.
        const prelude = `
          const assert = require('node:assert/strict');
          const { EventEmitterAsyncResource: E } = require('node:events');
          const { createHook } = require('node:async_hooks');
          const emitDestroy = E.prototype.emitDestroy;
          const make = (requireManualDestroy = true) =>
            new E({ name: 'emitDestroy-audit', requireManualDestroy });
          const symbol = (object, name) => {
            const key = Object.getOwnPropertySymbols(object)
              .find(key => key.description === name);
            assert.notEqual(key, undefined);
            return key;
          };
          const typeError = operation => assert.throws(operation, error => {
            assert.equal(error.constructor, TypeError);
            return true;
          });
          function checkPropagation(expected) {
            const hook = createHook({ destroy() {} }).enable();
            try {
              for (const branch of ['destroyed-first', 'destroyed-second', 'setter', 'asyncId', 'coercion']) {
                const ee = make(false);
                const resource = ee.asyncResource;
                const destroyed = symbol(resource, 'destroyed');
                const id = symbol(resource, 'async_id_symbol');
                const key = branch === 'asyncId' || branch === 'coercion' ? id : destroyed;
                const descriptor = Object.getOwnPropertyDescriptor(resource, key);
                let reads = 0;
                try {
                  if (branch === 'coercion') {
                    resource[id] = { [Symbol.toPrimitive]() { throw expected; } };
                  } else if (branch === 'setter') {
                    resource[destroyed] = { set destroyed(value) { throw expected; } };
                  } else {
                    Object.defineProperty(resource, key, { configurable: true, get() {
                      if (branch === 'destroyed-second' && ++reads === 1) return descriptor.value;
                      throw expected;
                    } });
                  }
                  let caught = false;
                  try { emitDestroy.call(ee); }
                  catch (error) {
                    caught = true;
                    assert.strictEqual(error, expected);
                    if (expected instanceof Error) assert.equal(error.constructor, expected.constructor);
                  }
                  assert.equal(caught, true, branch);
                } finally {
                  Object.defineProperty(resource, key, descriptor);
                  emitDestroy.call(ee);
                }
              }
            } finally { hook.disable(); }
          }
        `;

        async function runProbe(body: string) {
          const output = runNode(["-e", `${prelude}
            ${body}
          `]);
          expect(output).toBe(`verified ${process.version}`);
        }

        it("throws TypeError", { tags: ["node-only"] }, async () => {
          await runProbe(`
            const ee = make();
            let traps = 0;
            const proxy = new Proxy(ee, { get() { traps++; throw new Error('not reached'); } });
            const revoked = Proxy.revocable(ee, {});
            revoked.revoke();
            try {
              for (const receiver of [undefined, null, 1, 'text', true, 1n, Symbol(), {},
                Object.create(E.prototype), proxy, revoked.proxy]) {
                typeError(() => emitDestroy.call(receiver));
              }
              assert.equal(traps, 0);
            } finally { emitDestroy.call(ee); }
            const hook = createHook({ destroy() {} }).enable();
            try {
              const automatic = make(false);
              const resource = automatic.asyncResource;
              const destroyed = symbol(resource, 'destroyed');
              const id = symbol(resource, 'async_id_symbol');
              const originalBag = resource[destroyed];
              const originalId = resource[id];
              try {
                for (const bag of [null, 1, Object.freeze({ destroyed: false }),
                  Object.defineProperty({}, 'destroyed', { get() { return false; } }),
                  new Proxy({}, { set() { return false; } })]) {
                  resource[destroyed] = bag;
                  typeError(() => emitDestroy.call(automatic));
                }
                resource[destroyed] = originalBag;
                for (const value of [Symbol('invalid ID'), { [Symbol.toPrimitive]() { return {}; } }]) {
                  resource[id] = value;
                  typeError(() => emitDestroy.call(automatic));
                }
              } finally {
                resource[destroyed] = originalBag;
                resource[id] = originalId;
                emitDestroy.call(automatic);
              }
            } finally { hook.disable(); }
            process.stdout.write('verified ' + process.version);
          `);
        });

        it("throws Error", { tags: ["node-only"] }, async () => {
          await runProbe(`checkPropagation(new Error('supplied resource failure'));
            process.stdout.write('verified ' + process.version);`);
        });

        it("throws SyntaxError", { tags: ["node-only"] }, async () => {
          await runProbe(`checkPropagation(new SyntaxError('supplied resource failure'));
            process.stdout.write('verified ' + process.version);`);
        });

        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          await runProbe(`checkPropagation(Object.freeze({ supplied: true })); checkPropagation(undefined);
            process.stdout.write('verified ' + process.version);`);
        });

        it("returns before destroy delivery, including repeated calls", { tags: ["node-only"] }, async () => {
          await runProbe(`
            const noHooks = make(false);
            assert.equal(emitDestroy.call(noHooks), undefined);
            assert.equal(emitDestroy.call(noHooks), undefined);
            const seen = [];
            const hook = createHook({ destroy(id) { seen.push(id); } }).enable();
            const manual = make();
            const automatic = make(false);
            const ids = [manual.asyncId, automatic.asyncId];
            try {
              for (const ee of [manual, automatic]) {
                assert.equal(emitDestroy.call(ee), undefined);
                assert.equal(emitDestroy.call(ee), undefined);
              }
              assert.deepEqual(seen, []);
              const resource = automatic.asyncResource;
              assert.equal(resource[symbol(resource, 'destroyed')].destroyed, true);
            } catch (error) { hook.disable(); throw error; }
            setImmediate(() => {
              try {
                assert.deepEqual(seen.filter(id => ids.includes(id)), [ids[0], ids[0], ids[1], ids[1]]);
                process.stdout.write('verified ' + process.version);
              } finally { hook.disable(); }
            });
          `);
        });

        it("delivers throwing hooks fatally, not as synchronous throws", { tags: ["node-only"] }, async () => {
          const { spawnSync } = await import("node:child_process");
          for (const expression of ["new Error('destroy-hook-sentinel')", "new SyntaxError('destroy-hook-sentinel')", "'destroy-hook-sentinel'"]) {
            const result = spawnSync(process.execPath, ["-e", `${prelude}
              const fs = require('node:fs');
              const ee = make();
              const id = ee.asyncId;
              const supplied = ${expression};
              const hook = createHook({ destroy(current) {
                if (current === id) {
                  hook.disable();
                  fs.writeSync(1, 'delivered\\n');
                  throw supplied;
                }
              } }).enable();
              process.on('uncaughtException', () => fs.writeSync(1, 'unexpected uncaughtException\\n'));
              try { assert.equal(emitDestroy.call(ee), undefined); }
              finally { fs.writeSync(1, 'returned ' + process.version + '\\n'); }
              setImmediate(() => {});
            `], { encoding: "utf8", timeout: 10_000 });
            expect(result.error).toBeUndefined();
            expect(result.signal).toBeNull();
            expect(result.status).toBe(1);
            expect(result.stdout).toBe(`returned ${process.version}\ndelivered\n`);
            expect(result.stderr).toContain("destroy-hook-sentinel");
          }
        });
      });

      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("EventEmitterReferencingAsyncResource", () => {
      describe("eventEmitter", () => {
        describe("new", () => {
          it("TODO", () => {
            // TODO
          });
        });
      });
    });

    describe("NodeEventTarget", () => {
      describe("addListener", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("emit", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("eventNames", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("getMaxListeners", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("listenerCount", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("off", () => {
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

      describe("removeAllListeners", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("removeListener", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("setMaxListeners", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("[EventEmitter.captureRejectionSymbol]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static addAbortListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static getEventListeners", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static getMaxListeners", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static listenerCount", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static on", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static once", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static setMaxListeners", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Listener", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("Listener1", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("Listener2", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("global", () => {
    describe("NodeJS", () => {
      describe("EventEmitter", () => {
        describe("[EventEmitter.captureRejectionSymbol]", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("addListener", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("emit", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("eventNames", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("getMaxListeners", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("listenerCount", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("listeners", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("off", () => {
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

        describe("rawListeners", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("removeAllListeners", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("removeListener", () => {
          it("TODO", () => {
            // TODO
          });
        });

        describe("setMaxListeners", () => {
          it("TODO", () => {
            // TODO
          });
        });
      });
    });
  });
});
