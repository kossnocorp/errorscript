// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown } from "../../support/errors";
import { describe, expect, it } from "vitest";

describe("node:diagnostics_channel", () => {
  describe("Channel", () => {
    describe("bindStore", () => {
      const capture = captureThrown;

      it("throws TypeError", { tags: ["node-only"] }, async () => {
        const { Channel } = await import("node:diagnostics_channel");
        for (const receiver of [undefined, null, 1, "channel", true, Symbol()]) {
          const error = capture(() => Reflect.apply(Channel.prototype.bindStore, receiver, []));
          expect(error).toHaveExactConstructor(TypeError);
        }
      });

      describe("non-extensible inactive receiver", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { channel } = await import("node:diagnostics_channel");
          const target = channel(Symbol("bindStore non-extensible"));
          Object.preventExtensions(target);
          const error = capture(() => Reflect.apply(target.bindStore, target, []));
          expect(error).toHaveExactConstructor(TypeError);
          expect(target.hasSubscribers).toBe(false);
        });
      });

      describe("read-only activation property", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { channel } = await import("node:diagnostics_channel");
          const target = channel(Symbol("bindStore read-only"));
          const prototype = Object.getPrototypeOf(target);
          const descriptor = Object.getOwnPropertyDescriptor(target, "_subscribers")!;
          try {
            Object.defineProperty(target, "_subscribers", { writable: false });
            const error = capture(() => Reflect.apply(target.bindStore, target, []));
            expect(error).toHaveExactConstructor(TypeError);
          } finally {
            Object.defineProperty(target, "_subscribers", descriptor);
            Object.setPrototypeOf(target, prototype);
          }
        });
      });

      describe("active method incompatible receiver", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { channel } = await import("node:diagnostics_channel");
          const { AsyncLocalStorage } = await import("node:async_hooks");
          const target = channel(Symbol("bindStore active receiver"));
          const store = new AsyncLocalStorage();
          try {
            target.bindStore(store);
            for (const receiver of [null, undefined, {}, { _stores: {} }, { _stores: { has: Map.prototype.has } }]) {
              const error = capture(() => Reflect.apply(target.bindStore, receiver, [store]));
              expect(error).toHaveExactConstructor(TypeError);
            }
          } finally {
            target.unbindStore(store);
            store.disable();
          }
        });
      });

      describe("activation proxy trap", () => {
        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          const { Channel, channel } = await import("node:diagnostics_channel");
          const target = channel(Symbol("bindStore proxy activation"));
          const sentinel = { source: "setPrototypeOf" };
          const receiver = new Proxy(target, { setPrototypeOf() { throw sentinel; } });
          expect(capture(() => Reflect.apply(Channel.prototype.bindStore, receiver, []))).toBe(sentinel);
          expect(target.hasSubscribers).toBe(false);
        });
      });

      describe("active receiver property access", () => {
        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          const { channel } = await import("node:diagnostics_channel");
          const { AsyncLocalStorage } = await import("node:async_hooks");
          const target = channel(Symbol("bindStore proxy access"));
          const store = new AsyncLocalStorage();
          const other = new AsyncLocalStorage();
          const sentinel = { source: "get" };
          try {
            target.bindStore(store);
            for (const property of ["_stores", "name"]) {
              const receiver = new Proxy(target, {
                get(object, key, receiver) {
                  if (key === property) throw sentinel;
                  return Reflect.get(object, key, receiver);
                },
              });
              expect(capture(() => Reflect.apply(target.bindStore, receiver, [other]))).toBe(sentinel);
            }
          } finally {
            target.unbindStore(other);
            target.unbindStore(store);
            other.disable();
            store.disable();
          }
        });
      });

      it("throws RangeError", { tags: ["node-only"], timeout: 120_000 }, async () => {
        const { execFile } = await import("node:child_process");
        const { promisify } = await import("node:util");
        // Isolate the real V8 collection-capacity boundary and its substantial heap.
        const { stdout } = await promisify(execFile)(process.execPath, [
          "--max-old-space-size=2048",
          "--input-type=module",
          "-e",
          `import assert from 'node:assert/strict';
           import { channel } from 'node:diagnostics_channel';
           const target = channel(Symbol('bindStore capacity'));
           const capacity = 2 ** 24;
           let inserted = 0;
           try {
             // bindStore intentionally does not validate store arguments.
             for (; inserted < capacity; inserted++) target.bindStore(inserted);
             let caught;
             try { target.bindStore(capacity); } catch (error) { caught = error; }
             assert.equal(caught?.constructor, RangeError);
             console.log('RangeError:' + inserted);
           } finally {
             for (let i = 0; i < inserted; i++) target.unbindStore(i);
             assert.equal(target.hasSubscribers, false);
           }`,
        ], { timeout: 115_000 });
        expect(stdout.trim()).toBe("RangeError:16777216");
      });

      it("stores arguments without invoking or validating them", { tags: ["node-only"] }, async () => {
        const { channel } = await import("node:diagnostics_channel");
        // A string name also exercises the native subscriber-count index branch.
        const target = channel("errorscript.audit.bindStore.opaque-arguments");
        const sentinel = { source: "argument access" };
        const opaque = new Proxy({}, { get() { throw sentinel; } });
        const transform = () => { throw sentinel; };
        try {
          expect(Reflect.apply(target.bindStore, target, [opaque, transform])).toBeUndefined();
          expect(Reflect.apply(target.bindStore, target, [opaque, null])).toBeUndefined();
          expect(Reflect.apply(target.bindStore, target, [undefined, 1])).toBeUndefined();
        } finally {
          Reflect.apply(target.unbindStore, target, [opaque]);
          Reflect.apply(target.unbindStore, target, [undefined]);
        }
        expect(target.hasSubscribers).toBe(false);
      });
    });

    describe("publish", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("runStores", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("subscribe", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unbindStore", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unsubscribe", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ChannelListener", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("TracingChannel", () => {
    describe("subscribe", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("traceCallback", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("tracePromise", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("traceSync", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unsubscribe", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("TracingChannelSubscribers", () => {
    describe("asyncEnd", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("asyncStart", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("end", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("error", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("start", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("channel", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("hasSubscribers", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("subscribe", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("tracingChannel", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("unsubscribe", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
