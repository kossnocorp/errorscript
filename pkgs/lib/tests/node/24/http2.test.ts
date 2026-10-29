// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { runNode, nativeAssertions } from "../../support/node";

describe("node:http2", () => {
  describe("ClientHttp2Session", () => {
    describe("addListener", () => {
      // Real loopback HTTP/2 sessions; children isolate captureRejections and hooks.
      async function run(body: string, captureRejections = false, waitForConnect = true) {
        const output = runNode(["--input-type=commonjs", "-e", `
          const assert = require('node:assert/strict');
          const { EventEmitter } = require('node:events');
          EventEmitter.captureRejections = ${captureRejections};
          const http2 = require('node:http2');
          const noop = () => {};
          ${nativeAssertions}
          (async () => {
            assert.equal(process.version, 'v24.19.0');
            const server = http2.createServer();
            const peers = [];
            server.on('session', peer => {
              peer.on('error', noop);
              peers.push(peer);
            });
            let session;
            let closed;
            try {
              await new Promise((resolve, reject) => {
                server.once('error', reject);
                server.listen(0, '127.0.0.1', resolve);
              });
              session = http2.connect('http://127.0.0.1:' + server.address().port);
              closed = new Promise(resolve => session.once('close', resolve));
              const connected = new Promise(resolve => session.once('connect', resolve));
              assert.equal(session.addListener, EventEmitter.prototype.addListener);
              if (${waitForConnect}) await connected;
              ${body}
            } finally {
              if (session) {
                session.removeAllListeners('newListener');
                session.on('error', noop);
                session.destroy();
              }
              for (const peer of peers) peer.destroy();
              if (closed) await closed;
              await new Promise(resolve => server.close(resolve));
            }
            process.stdout.write('verified');
          })().catch(error => { console.error(error); process.exitCode = 1; });
        `]);
        expect(output).toBe("verified");
      }

      it("throws TypeError", async () => {
        await run(`
          for (const listener of [undefined, null, 1, 'listener', {}]) {
            exact(() => session.addListener('audit', listener), TypeError);
          }
          assert.equal(session.listenerCount('audit'), 0);
        `);
      });

      it("throws TypeError", async () => {
        await run(`
          for (const receiver of [null, undefined, 1, Object.freeze({})]) {
            exact(() => Reflect.apply(session.addListener, receiver, ['audit', noop]), TypeError);
          }
          const { proxy, revoke } = Proxy.revocable(session, {});
          revoke();
          exact(() => Reflect.apply(session.addListener, proxy, ['audit', noop]), TypeError);
          // A receiver with the real session's event hooks but no native fields.
          const receiver = { _events: session._events, emit: session.emit };
          exact(() => Reflect.apply(session.addListener, receiver, ['ping', noop]), TypeError);
        `);
      });

      it("throws TypeError", async () => {
        await run(`
          exact(() => session.addListener({ [Symbol.toPrimitive]() { return {}; } }, noop), TypeError);
          exact(() => session.addListener(Object.create(null), noop), TypeError);
          const symbol = Symbol('audit');
          const key = { [Symbol.toPrimitive]() { return symbol; } };
          session.setMaxListeners(1);
          session.addListener(key, noop);
          exact(() => session.addListener(key, noop), TypeError);
          assert.equal(session.listenerCount(symbol), 2);
        `);
      });

      it("throws ReferenceError", async () => {
        await run(`
          const { mkdtempSync, writeFileSync, rmSync } = require('node:fs');
          const { tmpdir } = require('node:os');
          const { join } = require('node:path');
          const { pathToFileURL } = require('node:url');
          const dir = mkdtempSync(join(tmpdir(), 'errs-http2-listener-'));
          try {
            globalThis.checkNamespace = namespace => {
              exact(() => session.addListener('audit', namespace), ReferenceError);
              assert.equal(session.listenerCount('audit'), 0);
            };
            const file = join(dir, 'listener.mjs');
            writeFileSync(file, "import * as ns from './listener.mjs'; globalThis.checkNamespace(ns); export let constructor;");
            await import(pathToFileURL(file).href);
          } finally {
            delete globalThis.checkNamespace;
            rmSync(dir, { recursive: true, force: true });
          }
        `);
      });

      it("throws RangeError", async () => {
        await run(`
          const listeners = [];
          listeners.length = 2 ** 32 - 1;
          const receiver = { _events: { audit: listeners }, _eventsCount: 1 };
          exact(() => Reflect.apply(session.addListener, receiver, ['audit', noop]), RangeError);
        `);
      });

      it("throws Error", async () => {
        await run(`
          const value = { audit: 'then getter' };
          session.on('newListener', event => event === 'audit' ? { get then() { throw value; } } : undefined);
          assert.throws(() => session.addListener('audit', noop), error => {
            assert.equal(error.constructor, Error);
            assert.equal(error.context, value);
            return true;
          });
          assert.equal(session.listenerCount('audit'), 0);
        `, true);
      });

      it("throws Error", async () => {
        await run(`
          const error = new Error('supplied newListener failure');
          session.on('newListener', event => { if (event === 'audit') throw error; });
          same(() => session.addListener('audit', noop), error);
          assert.equal(error.constructor, Error);
          assert.equal(session.listenerCount('audit'), 0);
        `);
      });

      it("throws SyntaxError", async () => {
        await run(`
          const error = new SyntaxError('supplied newListener failure');
          session.on('newListener', event => { if (event === 'audit') throw error; });
          same(() => session.addListener('audit', noop), error);
          assert.equal(error.constructor, SyntaxError);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const value = Symbol('supplied callback failure');
          session.on('newListener', event => { if (event === 'audit') throw value; });
          same(() => session.addListener('audit', noop), value);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const value = { audit: 'supplied hook failure' };
          same(() => session.addListener({ [Symbol.toPrimitive]() { throw value; } }, noop), value);
          same(() => session.addListener('audit', { get constructor() { throw value; } }), value);
          const receiver = new Proxy(session, { get(target, key, receiver) {
            if (key === '_events') throw value;
            return Reflect.get(target, key, receiver);
          } });
          same(() => Reflect.apply(session.addListener, receiver, ['audit', noop]), value);
          const listener = Object.defineProperty(() => {}, 'listener', { get() { throw value; } });
          same(() => session.addListener('audit', listener), value);
        `);
      });

      it("throws non-Error value", async () => {
        await run(`
          const { inspect } = require('node:util');
          const value = { audit: 'supplied inspect failure' };
          session[inspect.custom] = () => { throw value; };
          try {
            session.setMaxListeners(1);
            session.addListener('audit', noop);
            same(() => session.addListener('audit', noop), value);
            assert.equal(session.listenerCount('audit'), 2);
          } finally {
            delete session[inspect.custom];
          }
        `);
      });

      it("delivers a warning, not a thrown Error", async () => {
        await run(`
          const delivered = new Promise(resolve => process.once('warning', resolve));
          session.setMaxListeners(1);
          session.addListener('audit', noop);
          assert.equal(session.addListener('audit', noop), session);
          const warning = await delivered;
          assert.equal(warning.constructor, Error);
          assert.equal(warning.name, 'MaxListenersExceededWarning');
          assert.equal(warning.emitter, session);
          assert.equal(warning.type, 'audit');
          assert.equal(warning.count, 2);
        `);
      });

      it("delivers captured newListener rejection by destroying the session", async () => {
        await run(`
          const error = new Error('supplied rejection');
          const delivered = new Promise(resolve => session.once('error', resolve));
          session.on('newListener', event => event === 'audit' ? Promise.reject(error) : undefined);
          assert.equal(session.addListener('audit', noop), session);
          assert.equal(session.destroyed, false);
          assert.equal(await delivered, error);
          assert.equal(error.constructor, Error);
          await closed;
          assert.equal(session.destroyed, true);
        `, true);
      });

      it("delivers a numeric captured rejection as ERR_HTTP2_SESSION_ERROR", async () => {
        await run(`
          const delivered = new Promise(resolve => session.once('error', resolve));
          session.on('newListener', event => event === 'audit' ? Promise.reject(1) : undefined);
          assert.equal(session.addListener('audit', noop), session);
          const error = await delivered;
          assert.equal(error.constructor, Error);
          await closed;
          assert.equal(session.destroyed, true);
        `, true);
      });

      it("delivers a non-Error captured rejection by identity", async () => {
        await run(`
          const value = { audit: 'supplied rejection' };
          const delivered = new Promise(resolve => session.once('error', resolve));
          session.on('newListener', event => event === 'audit' ? Promise.reject(value) : undefined);
          assert.equal(session.addListener('audit', noop), session);
          assert.equal(await delivered, value);
          await closed;
          assert.equal(session.destroyed, true);
        `, true);
      });

      it("maps native listener hooks before connect, when connected, and after destroy", async () => {
        await run(`
          const names = ['ping', 'altsvc', 'remoteSettings', 'priority', 'frameError'];
          assert.equal(session.connecting, true);
          for (const name of names) assert.equal(session.addListener(name, noop), session);
          await connected;
          for (const name of names) assert.equal(session.addListener(name, noop), session);
          session.destroy();
          await closed;
          for (const name of names) {
            assert.equal(session.addListener(name, noop), session);
            assert.equal(session.listenerCount(name), 3);
          }
        `, false, false);
      });

      it("maps inherited overloads without invoking the newly registered callback", async () => {
        await run(`
          const error = new Error('only a later emission invokes this');
          const names = ['altsvc', 'origin', 'connect', 'stream', 'close', 'error',
            'frameError', 'goaway', 'localSettings', 'ping', 'remoteSettings', 'timeout', Symbol('audit')];
          for (const name of names) assert.equal(session.addListener(name, noop), session);
          assert.equal(session.addListener('audit', () => { throw error; }), session);
          session.addListener('audit', noop);
          session.addListener('audit', noop);
          assert.equal(session.listenerCount('audit'), 3);
          same(() => session.emit('audit'), error); // Later emit, not registration.
          // Adding during active newListener-array emission exercises copy-on-write.
          let reentered = false;
          session.on('newListener', event => {
            if (event === 'audit' && !reentered) {
              reentered = true;
              session.addListener('newListener', noop);
            }
          });
          session.addListener('audit', noop);
          assert.equal(reentered, true);
        `);
      });

      it("throws RangeError", async () => {
        await run(`
          const { inspect } = require('node:util');
          const left = { constructor: null, [inspect.custom]: () => right };
          const right = { [inspect.custom]: () => left };
          // Neither hook throws or calls inspect: Node follows their return cycle.
          exact(() => session.addListener('audit', left), RangeError);
          assert.equal(session.listenerCount('audit'), 0);
          session[inspect.custom] = () => left;
          try {
            session.setMaxListeners(1);
            session.addListener('audit', noop);
            exact(() => session.addListener('audit', noop), RangeError);
            assert.equal(session.listenerCount('audit'), 2);
          } finally {
            delete session[inspect.custom];
          }
        `);
      });

      it("throws Error", async () => {
        await run(`
          const { inspect } = require('node:util');
          const source = 'export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
          const namespace = await import('data:text/javascript,' + encodeURIComponent(source));
          const named = function () {};
          Object.defineProperty(named, 'name', { value: namespace });
          const invalid = { constructor: null, [inspect.custom]: () => named };
          const internalAssertion = call => assert.throws(call, error => {
            assert.equal(error.constructor, Error);
            assert.notEqual(error, 17); // Node replaces the exported hook's throw.
            return true;
          });
          internalAssertion(() => session.addListener('audit', invalid));
          assert.equal(session.listenerCount('audit'), 0);
          session[inspect.custom] = () => named;
          try {
            session.setMaxListeners(1);
            session.addListener('audit', noop);
            internalAssertion(() => session.addListener('audit', noop));
            assert.equal(session.listenerCount('audit'), 2);
          } finally {
            delete session[inspect.custom];
          }
        `);
      });
    });

    describe("emit", () => {
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

    describe("request", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ClientHttp2Stream", () => {
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

  describe("ClientSessionOptions", () => {
    describe("createConnection", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("HTTP2ServerCommon", () => {
    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("updateSettings", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Http2SecureServer", () => {
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

  describe("Http2Server", () => {
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

  describe("Http2ServerRequest", () => {
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

    describe("read", () => {
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

  describe("Http2ServerResponse", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

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

    describe("createPushResponse", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("end", () => {
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

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("write", () => {
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
  });

  describe("Http2Session", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("close", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("destroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("goaway", () => {
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

    describe("ping", () => {
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

    describe("setLocalWindowSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("settings", () => {
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

  describe("Http2Stream", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("close", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
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

    describe("priority", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("sendTrailers", () => {
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

  describe("ServerHttp2Session", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("altsvc", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("emit", () => {
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

    describe("origin", () => {
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

  describe("ServerHttp2Stream", () => {
    describe("additionalHeaders", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("pushStream", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("respond", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("respondWithFD", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("respondWithFile", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ServerStreamFileResponseOptions", () => {
    describe("statCheck", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ServerStreamFileResponseOptionsWithError", () => {
    describe("onError", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("connect", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createSecureServer", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createServer", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getDefaultSettings", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getPackedSettings", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getUnpackedSettings", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("performServerHandshake", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
