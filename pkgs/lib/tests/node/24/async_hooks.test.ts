// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { runNode } from "../../support/node";

describe("node:async_hooks", () => {
  describe("AsyncHook", () => {
    describe("disable", () => {
      // Keep hook bookkeeping (including deliberately failing reads) out of Vitest.
      async function runProbe(body: string) {
        const output = runNode(["-e", `
          const assert = require('node:assert/strict');
          const { createHook, AsyncResource } = require('node:async_hooks');
          const { enable, disable } = Object.getPrototypeOf(createHook({}));
          function checkReads(expected, invariant = false) {
            const names = ['init', 'before', 'after', 'destroy', 'promiseResolve'];
            const symbols = Object.getOwnPropertySymbols(createHook({}))
              .filter(key => names.includes(key.description));
            assert.equal(symbols.length, 5);
            for (const symbol of symbols) {
              const target = Object.freeze(createHook({}));
              let armed = false;
              const receiver = new Proxy(target, {
                get(target, key, receiver) {
                  if (armed && key === symbol) {
                    if (invariant) return true;
                    throw expected;
                  }
                  return Reflect.get(target, key, receiver);
                },
              });
              enable.call(receiver);
              try {
                armed = true;
                let caught = false;
                try {
                  disable.call(receiver);
                } catch (error) {
                  caught = true;
                  if (invariant) {
                    assert.equal(error.constructor, TypeError);
                  } else {
                    assert.strictEqual(error, expected);
                  }
                }
                assert.equal(caught, true);
              } finally {
                armed = false;
                assert.strictEqual(disable.call(receiver), receiver);
              }
            }
          }
          ${body}
          process.stdout.write('verified ' + process.version);
        `]);
        expect(output).toBe(`verified ${process.version}`);
      }

      it("throws TypeError", { tags: ["node-only"] }, async () => {
        // The native Proxy invariant check throws; the get trap returns normally.
        await runProbe(`
          checkReads(undefined, true);
          const handler = {};
          const receiver = new Proxy(createHook({}), handler);
          enable.call(receiver);
          try {
            handler.get = 1;
            assert.throws(() => disable.call(receiver), error => {
              assert.equal(error.constructor, TypeError);
              return true;
            });
          } finally {
            delete handler.get;
            disable.call(receiver);
          }
        `);
      });

      it("propagates supplied effect", { tags: ["node-only"] }, async () => {
        await runProbe("checkReads(Object.freeze({ supplied: true }));");
      });

      it("returns the receiver without brand checks", { tags: ["node-only"] }, async () => {
        await runProbe(`
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          for (const receiver of [undefined, null, 1, 'text', {}, proxy])
            assert.strictEqual(disable.call(receiver), receiver);
          const hook = createHook({ init() {} });
          try {
            assert.strictEqual(hook.disable(), hook);
            hook.enable();
            assert.strictEqual(hook.disable(), hook);
            assert.strictEqual(hook.disable(), hook);
          } finally {
            hook.disable();
          }
          let calls = 0;
          let callbackError;
          const selfDisabling = createHook({ init() {
            calls++;
            try { assert.strictEqual(selfDisabling.disable(), selfDisabling); }
            catch (error) { callbackError = error; }
          } });
          try {
            selfDisabling.enable();
            new AsyncResource('disable-audit');
            new AsyncResource('disable-audit');
            assert.equal(callbackError, undefined);
            assert.equal(calls, 1);
          } finally {
            selfDisabling.disable();
          }
        `);
      });
    });

    describe("enable", () => {
      // Isolate global hook counters and deliberately failing receiver reads.
      async function runProbe(body: string) {
        const output = runNode(["-e", `
          const assert = require('node:assert/strict');
          const { createHook, AsyncResource } = require('node:async_hooks');
          const { enable, disable } = Object.getPrototypeOf(createHook({}));
          const symbols = Object.getOwnPropertySymbols(createHook({}));
          assert.deepEqual(symbols.map(key => key.description),
            ['init', 'before', 'after', 'destroy', 'promiseResolve', 'kNoPromiseHook']);
          function typeError(operation) {
            assert.throws(operation, error => {
              assert.equal(error.constructor, TypeError);
              return true;
            });
          }
          function checkReads(expected, mode = 'proxy') {
            for (const symbol of symbols) {
              const target = createHook({});
              let armed = true;
              let receiver;
              if (mode === 'getter') {
                const original = target[symbol];
                Object.defineProperty(target, symbol, { get() {
                  if (armed) throw expected;
                  return original;
                } });
                receiver = target;
              } else {
                Object.freeze(target);
                receiver = new Proxy(target, { get(target, key, receiver) {
                  if (armed && key === symbol) {
                    if (mode === 'invariant') return true;
                    throw expected;
                  }
                  return Reflect.get(target, key, receiver);
                } });
              }
              try {
                let caught = false;
                try {
                  enable.call(receiver);
                } catch (error) {
                  caught = true;
                  if (mode === 'invariant') {
                    assert.equal(error.constructor, TypeError);
                  } else {
                    assert.strictEqual(error, expected);
                  }
                }
                assert.equal(caught, true);
              } finally {
                armed = false;
                // A failure on kNoPromiseHook happens AFTER registration.
                assert.strictEqual(disable.call(receiver), receiver);
              }
            }
          }
          ${body}
          process.stdout.write('verified ' + process.version);
        `]);
        expect(output).toBe(`verified ${process.version}`);
      }

      it("throws TypeError", { tags: ["node-only"] }, async () => {
        await runProbe(`
          for (const receiver of [undefined, null])
            typeError(() => enable.call(receiver));
          const { proxy, revoke } = Proxy.revocable(createHook({}), {});
          revoke();
          typeError(() => enable.call(proxy));
          typeError(() => enable.call(new Proxy(createHook({}), { get: 1 })));
          checkReads(undefined, 'invariant');
        `);
      });

      it("propagates supplied effect", { tags: ["node-only"] }, async () => {
        await runProbe(`
          const supplied = Object.freeze({ supplied: true });
          checkReads(supplied);
          checkReads(supplied, 'getter');
        `);
      });

      it("returns the receiver without brand checks", { tags: ["node-only"] }, async () => {
        await runProbe(`
          for (const receiver of [1, 'text', true, 1n, Symbol('receiver'), {}]) {
            try {
              assert.strictEqual(enable.call(receiver), receiver);
              assert.strictEqual(enable.call(receiver), receiver);
            } finally {
              disable.call(receiver);
            }
          }
          let armed = false;
          const receiver = new Proxy(createHook({}), { get(target, key, receiver) {
            if (armed) throw new Error('already-enabled receiver must not be read');
            return Reflect.get(target, key, receiver);
          } });
          try {
            enable.call(receiver);
            armed = true;
            assert.strictEqual(enable.call(receiver), receiver);
          } finally {
            armed = false;
            disable.call(receiver);
          }
        `);
      });

      it("returns through hook and promise-mode transitions", { tags: ["node-only"] }, async () => {
        await runProbe(`
          let callbacks = 0;
          const callback = () => { callbacks++; };
          const configurations = [
            {}, { init: callback }, { destroy: callback },
            { init: callback, destroy: callback }, { promiseResolve: callback },
            { before: callback, after: callback },
            { init: callback, trackPromises: false },
          ];
          for (const configuration of configurations) {
            const hook = createHook(configuration);
            const additional = createHook(configuration);
            try {
              assert.strictEqual(hook.enable(), hook);
              assert.strictEqual(hook.enable(), hook);
              assert.strictEqual(additional.enable(), additional);
              assert.equal(callbacks, 0); // Registration never invokes callbacks.
            } finally {
              additional.disable();
              hook.disable();
            }
          }
          let addedCalls = 0;
          let callbackError;
          const added = createHook({ init() { addedCalls++; } });
          const outer = createHook({ init() {
            try { assert.strictEqual(added.enable(), added); }
            catch (error) { callbackError = error; }
          } });
          try {
            outer.enable();
            new AsyncResource('enable-audit');
            assert.equal(addedCalls, 0);
            new AsyncResource('enable-audit');
            assert.equal(addedCalls, 1);
            assert.equal(callbackError, undefined);
          } finally {
            outer.disable();
            added.disable();
          }
        `);
      });
    });
  });

  describe("AsyncLocalStorage", () => {
    describe("disable", () => {
      // Test both real Node implementations without changing Vitest's own context.
      async function runProbe(legacy: boolean, body: string) {
        const flag = legacy ? "--no-async-context-frame" : "--async-context-frame";
        const output = runNode([flag, "-e", `
          const assert = require('node:assert/strict');
          const { AsyncLocalStorage, AsyncResource, createHook } = require('node:async_hooks');
          const { disable } = AsyncLocalStorage.prototype;
          assert.equal(disable.toString().includes('this.enabled'), ${legacy});
          function typeError(operation) {
            assert.throws(operation, error => {
              assert.equal(error.constructor, TypeError);
              return true;
            });
          }
          function checkPropagation(supplied) {
            const receivers = [
              { get enabled() { throw supplied; } },
              { get enabled() { return true; }, set enabled(value) { throw supplied; } },
              new Proxy({}, { get(target, key, receiver) {
                if (key === 'enabled') throw supplied;
                return Reflect.get(target, key, receiver);
              } }),
              new Proxy({ enabled: true }, { set(target, key, value, receiver) {
                if (key === 'enabled') throw supplied;
                return Reflect.set(target, key, value, receiver);
              } }),
            ];
            for (const receiver of receivers) {
              let caught = false;
              try { disable.call(receiver); }
              catch (error) {
                caught = true;
                assert.strictEqual(error, supplied);
              }
              assert.equal(caught, true);
            }
          }
          ${body}
          process.stdout.write('verified ' + process.version + ' ' + ${JSON.stringify(flag)});
        `]);
        expect(output).toBe(`verified ${process.version} ${flag}`);
      }

      it("returns without receiver access in default frame mode", { tags: ["node-only"] }, async () => {
        await runProbe(false, `
          const poison = new Proxy({}, { get() { throw new Error('unexpected read'); } });
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          const receivers = [undefined, null, 1, 1n, Symbol(), 'text', {}, poison, proxy];
          function check() {
            for (const receiver of receivers)
              assert.equal(disable.call(receiver, poison), undefined);
          }
          check(); // No current frame.
          const storage = new AsyncLocalStorage();
          try {
            storage.run('store', () => {
              check(); // Existing frame: every receiver is only a Map key.
              assert.equal(storage.getStore(), 'store');
              assert.equal(storage.disable(), undefined);
              assert.equal(storage.getStore(), undefined);
              assert.equal(storage.disable(), undefined);
            });
          } finally { storage.disable(); }
        `);
      });

      it("throws TypeError", { tags: ["node-only"] }, async () => {
        await runProbe(true, `
          for (const receiver of [undefined, null])
            typeError(() => disable.call(receiver));
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          typeError(() => disable.call(proxy));
          const frozen = Object.freeze({ enabled: true });
          const receivers = [
            frozen,
            { get enabled() { return true; } },
            new Proxy({}, { get: 1 }),
            new Proxy({ enabled: true }, { set: 1 }),
            new Proxy({ enabled: true }, { set() { return false; } }),
            new Proxy(frozen, { get() { return false; } }),
            new Proxy(frozen, { set() { return true; } }),
          ];
          for (const receiver of receivers)
            typeError(() => disable.call(receiver));
        `);
      });

      it("throws ReferenceError", { tags: ["node-only"] }, async () => {
        const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
        const { tmpdir } = await import("node:os");
        const { join } = await import("node:path");
        const directory = await mkdtemp(join(tmpdir(), "als-disable-namespace-"));
        try {
          const file = join(directory, "probe.mjs");
          await writeFile(file, `
            import assert from 'node:assert/strict';
            import { AsyncLocalStorage } from 'node:async_hooks';
            import * as namespace from './probe.mjs';
            const legacy = process.execArgv.includes('--no-async-context-frame');
            const { disable } = AsyncLocalStorage.prototype;
            assert.equal(disable.toString().includes('this.enabled'), legacy);
            const receivers = [namespace, Object.create(namespace), new Proxy(namespace, {})];
            // The namespace binding exists; its exported enabled binding is still in TDZ.
            for (const receiver of receivers) {
              if (legacy) {
                assert.throws(() => disable.call(receiver), error => {
                  assert.equal(error.constructor, ReferenceError);
                  return true;
                });
              } else {
                assert.equal(disable.call(receiver), undefined);
              }
            }
            if (!legacy) {
              const storage = new AsyncLocalStorage();
              try {
                storage.run('store', () => {
                  for (const receiver of receivers)
                    assert.equal(disable.call(receiver), undefined);
                  assert.equal(storage.getStore(), 'store');
                });
              } finally { storage.disable(); }
            }
            export let enabled = false;
            // Same native namespace now succeeds: no getter or supplied error was used.
            for (const receiver of receivers)
              assert.equal(disable.call(receiver), undefined);
            process.stdout.write('verified ' + process.version + ' namespace ' + legacy);
          `);
          for (const legacy of [true, false]) {
            const flag = legacy ? "--no-async-context-frame" : "--async-context-frame";
            const output = runNode([flag, file]);
            expect(output).toBe(`verified ${process.version} namespace ${legacy}`);
          }
        } finally {
          await rm(directory, { recursive: true, force: true });
        }
      });

      it("propagates supplied effect", { tags: ["node-only"] }, async () => {
        await runProbe(true, "checkPropagation(Object.freeze({ supplied: true }));");
      });

      it("returns through legacy storage and hook transitions", { tags: ["node-only"] }, async () => {
        await runProbe(true, `
          for (const receiver of [{}, 1, 1n, Symbol(), 'text', { enabled: false }])
            assert.equal(disable.call(receiver), undefined);
          const first = new AsyncLocalStorage();
          const second = new AsyncLocalStorage();
          try {
            first.enterWith('first');
            second.enterWith('second');
            assert.equal(first.disable(), undefined); // Other storage remains.
            assert.equal(second.getStore(), 'second');
            assert.equal(first.getStore(), undefined);
            assert.equal(first.disable(), undefined); // Already disabled.
            assert.equal(second.disable(), undefined); // Last storage hook.
          } finally { first.disable(); second.disable(); }
          const storage = new AsyncLocalStorage();
          let calls = 0;
          let callbackError;
          const hook = createHook({ init() {
            calls++;
            try { assert.equal(storage.disable(), undefined); }
            catch (error) { callbackError = error; }
          } });
          try {
            hook.enable();
            new AsyncResource('als-disable-audit');
            assert.equal(calls, 1);
            assert.equal(callbackError, undefined);
            assert.equal(storage.getStore(), undefined);
          } finally { storage.disable(); hook.disable(); }
        `);
      });
    });

    describe("enterWith", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("exit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getStore", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("run", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static bind", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static snapshot", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("AsyncResource", () => {
    describe("asyncId", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("bind", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emitDestroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("runInAsyncScope", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static bind", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("triggerAsyncId", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("HookCallbacks", () => {
    describe("after", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("before", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("destroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("init", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("promiseResolve", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("createHook", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("executionAsyncId", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("executionAsyncResource", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("triggerAsyncId", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
