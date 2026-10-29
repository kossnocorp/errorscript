// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { runNode, nativeAssertions } from "../../support/node";

describe("node:cluster", () => {
  describe("Cluster", () => {
    describe("addListener", () => {
      // Each real-Node child owns its cluster singleton, listeners, and settings.
      async function run(body: string, captureRejections = false) {
        const output = runNode(["--input-type=commonjs", "-e", `
          const assert = require('node:assert/strict');
          const { EventEmitter, captureRejectionSymbol } = require('node:events');
          EventEmitter.captureRejections = ${captureRejections};
          const cluster = require('node:cluster');
          const { inspect } = require('node:util');
          assert.equal(process.version, 'v24.19.0');
          assert.equal(cluster.addListener, EventEmitter.prototype.addListener);
          const noop = () => {};
          ${nativeAssertions}
          (async () => {
            ${body}
            process.stdout.write('verified');
          })().catch(error => { console.error(error); process.exitCode = 1; });
        `]);
        expect(output).toBe("verified");
      }

      it("throws TypeError", async () => {
        await run(`
          for (const listener of [undefined, null, 1, 'listener', {}]) {
            exact(() => cluster.addListener('audit', listener), TypeError);
          }
          assert.equal(cluster.listenerCount('audit'), 0);
        `);
      });

      it("throws TypeError", async () => {
        await run(`
          for (const receiver of [null, undefined, 1, Object.freeze({})]) {
            exact(() => Reflect.apply(cluster.addListener, receiver, ['audit', noop]), TypeError);
          }
          const { proxy, revoke } = Proxy.revocable(cluster, {});
          revoke();
          exact(() => Reflect.apply(cluster.addListener, proxy, ['audit', noop]), TypeError);
        `);
      });

      it("throws ReferenceError", async () => {
        await run(`
          const { mkdtempSync, writeFileSync, rmSync } = require('node:fs');
          const { join } = require('node:path');
          const { tmpdir } = require('node:os');
          const { pathToFileURL } = require('node:url');
          const directory = mkdtempSync(join(tmpdir(), 'cluster-addlistener-'));
          try {
            const file = join(directory, 'fixture.mjs');
            writeFileSync(file, [
              "import * as listener from './fixture.mjs';",
              "import cluster from 'node:cluster';",
              "import assert from 'node:assert/strict';",
              "assert.throws(() => cluster.addListener('audit', listener), error => {",
              "  assert.equal(error.constructor, ReferenceError);",
              "  return true;",
              "});",
              "export let constructor;",
              "export const verified = true;",
            ].join(' '));
            const fixture = await import(pathToFileURL(file).href);
            assert.equal(fixture.verified, true); // Native fixture import itself succeeds.
            assert.equal(cluster.listenerCount('audit'), 0);
          } finally {
            rmSync(directory, { recursive: true, force: true });
          }
        `);
      });

      it("throws TypeError", async () => {
        await run(`
          const source = [
            'export let calls = 0;',
            'export const value = {',
            '  [Symbol.for("nodejs.util.inspect.custom")]() { calls++; throw 17; }',
            '};',
          ].join(' ');
          const namespace = await import('data:text/javascript,' + encodeURIComponent(source));
          const invalid = { constructor: null, [inspect.custom]: () => namespace };
          exact(() => cluster.addListener('audit', invalid), TypeError);
          // Direct namespace returns stop here; function-name detours are tested below.
          cluster[inspect.custom] = () => namespace;
          cluster.setMaxListeners(1);
          const delivered = new Promise(resolve => process.once('warning', resolve));
          cluster.addListener('audit', noop);
          assert.equal(cluster.addListener('audit', noop), cluster);
          const warning = await delivered;
          assert.equal(warning.constructor, Error);
          assert.equal(warning.name, 'MaxListenersExceededWarning');
          assert.equal(namespace.calls, 0);
        `);
      });

      it("throws Error", async () => {
        await run(`
          const source = 'export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
          const namespace = await import('data:text/javascript,' + encodeURIComponent(source));
          const named = function () {};
          Object.defineProperty(named, 'name', { value: namespace });
          const invalid = { constructor: null, [inspect.custom]: () => named };
          const internalAssertion = call => assert.throws(call, error => {
            assert.equal(error.constructor, Error);
            assert.notEqual(error, 17); // Node replaces the hook's primitive throw.
            return true;
          });
          internalAssertion(() => cluster.addListener('audit', invalid));
          assert.equal(cluster.listenerCount('audit'), 0);
          cluster[inspect.custom] = () => named;
          cluster.setMaxListeners(1);
          cluster.addListener('audit', noop);
          internalAssertion(() => cluster.addListener('audit', noop));
          assert.equal(cluster.listenerCount('audit'), 2);
        `);
      });

      it("throws RangeError", async () => {
        await run(`
          const left = { constructor: null, [inspect.custom]: () => right };
          const right = { [inspect.custom]: () => left };
          // Hooks return normally; inspect itself recurses through their return cycle.
          exact(() => cluster.addListener('audit', left), RangeError);
          cluster[inspect.custom] = () => left;
          cluster.setMaxListeners(1);
          cluster.addListener('audit', noop);
          exact(() => cluster.addListener('audit', noop), RangeError);
          assert.equal(cluster.listenerCount('audit'), 2);
        `);
      });

      it("throws TypeError", async () => {
        await run(`
          exact(() => cluster.addListener({ [Symbol.toPrimitive]() { return {}; } }, noop), TypeError);
          exact(() => cluster.addListener(Object.create(null), noop), TypeError);
          // Property-key conversion accepts a symbol, but warning String(object) does not.
          const symbol = Symbol('audit');
          const key = { [Symbol.toPrimitive]() { return symbol; } };
          cluster.setMaxListeners(1);
          cluster.addListener(key, noop);
          exact(() => cluster.addListener(key, noop), TypeError);
          assert.equal(cluster.listenerCount(symbol), 2); // Mutation precedes warning formatting.
        `);
      });

      it("throws Error", async () => {
        await run(`
          const value = { audit: 'then getter' };
          cluster.on('newListener', () => ({ get then() { throw value; } }));
          assert.throws(() => cluster.addListener('audit', noop), error => {
            assert.equal(error.constructor, Error);
            assert.equal(error.context, value);
            return true;
          });
          assert.equal(cluster.listenerCount('audit'), 0);
        `, true);
      });

      it("throws RangeError", async () => {
        await run(`
          // addListener is generic: a forged receiver can expose a full sparse array.
          const listeners = [];
          listeners.length = 2 ** 32 - 1;
          const receiver = { _events: { audit: listeners }, _eventsCount: 1 };
          exact(() => Reflect.apply(cluster.addListener, receiver, ['audit', noop]), RangeError);
        `);
      });

      it("throws Error", async () => {
        await run(`
          const error = new Error('supplied newListener failure');
          cluster.on('newListener', event => { if (event === 'audit') throw error; });
          same(() => cluster.addListener('audit', noop), error);
          assert.equal(error.constructor, Error);
          assert.equal(cluster.listenerCount('audit'), 0);
        `);
      });

      it("throws SyntaxError", async () => {
        await run(`
          const error = new SyntaxError('supplied newListener failure');
          cluster.on('newListener', noop);
          cluster.on('newListener', event => { if (event === 'audit') throw error; });
          same(() => cluster.addListener('audit', noop), error);
          assert.equal(error.constructor, SyntaxError);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const value = Symbol('supplied callback failure');
          cluster.on('newListener', () => { throw value; });
          same(() => cluster.addListener('audit', noop), value);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const value = { audit: 'supplied hook failure' };
          same(() => cluster.addListener({ [Symbol.toPrimitive]() { throw value; } }, noop), value);
          const invalid = { get constructor() { throw value; } };
          same(() => cluster.addListener('audit', invalid), value);
          const receiver = new Proxy(cluster, { get(target, key, receiver) {
            if (key === '_events') throw value;
            return Reflect.get(target, key, receiver);
          } });
          same(() => Reflect.apply(cluster.addListener, receiver, ['audit', noop]), value);
          cluster.on('newListener', noop);
          const listener = Object.defineProperty(() => {}, 'listener', { get() { throw value; } });
          same(() => cluster.addListener('audit', listener), value);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const value = { audit: 'supplied inspect failure' };
          cluster[inspect.custom] = () => { throw value; };
          cluster.setMaxListeners(1);
          cluster.addListener('audit', noop);
          same(() => cluster.addListener('audit', noop), value);
          assert.equal(cluster.listenerCount('audit'), 2);
        `);
      });

      it("delivers a warning, not a thrown Error", async () => {
        await run(`
          const delivered = new Promise(resolve => process.once('warning', resolve));
          cluster.setMaxListeners(1);
          cluster.addListener('audit', noop);
          assert.equal(cluster.addListener('audit', noop), cluster);
          const warning = await delivered;
          assert.equal(warning.constructor, Error);
          assert.equal(warning.name, 'MaxListenersExceededWarning');
          assert.equal(warning.emitter, cluster);
          assert.equal(warning.type, 'audit');
          assert.equal(warning.count, 2);
        `);
      });

      it("delivers captured newListener rejection asynchronously", async () => {
        await run(`
          const value = { audit: 'supplied rejection' };
          const delivered = new Promise(resolve => cluster.once('error', resolve));
          cluster.on('newListener', event => event === 'audit' ? Promise.reject(value) : undefined);
          assert.equal(cluster.addListener('audit', noop), cluster);
          assert.equal(await delivered, value);
          // The alternative capture handler receives the original rejection and event.
          const captured = new Promise(resolve => {
            cluster[captureRejectionSymbol] = (error, event, ...args) => resolve({ error, event, args });
          });
          cluster.addListener('audit', noop);
          const result = await captured;
          assert.equal(result.error, value);
          assert.equal(result.event, 'newListener');
          assert.deepEqual(result.args, ['audit', noop]);
        `, true);
      });

      it("maps inherited registration without invoking the registered callback", async () => {
        await run(`
          const error = new Error('only a later emission invokes this');
          assert.equal(cluster.addListener('audit', () => { throw error; }), cluster);
          // Exercise first, second, and subsequent listener-array registration.
          assert.equal(cluster.addListener('audit', noop), cluster);
          assert.equal(cluster.addListener('audit', noop), cluster);
          assert.equal(cluster.listenerCount('audit'), 3);
          same(() => cluster.emit('audit'), error); // Later emit, not addListener.
        `);
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

    describe("fork", () => {
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

    describe("setupMaster", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setupPrimary", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ClusterSettings", () => {
    describe("inspectPort", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Worker", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("destroy", () => {
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

    describe("isConnected", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("isDead", () => {
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

    describe("send", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("cluster", () => {
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

    describe("fork", () => {
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

    describe("setupMaster", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setupPrimary", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });
});
