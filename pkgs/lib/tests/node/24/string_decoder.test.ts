// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, throwOnGet } from "../../support/errors";
import { invalidArgumentEffects } from "../../support/node-diagnostics";
import { runNode } from "../../support/node";
import { describe, expect, it } from "vitest";

describe("node:string_decoder", () => {
  describe("StringDecoder", () => {
    describe("end", () => {
      const thrown = captureThrown;

      describe("argument validation", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const decoder = new StringDecoder();
          // Primitive validation, object diagnostics, and a buffer that is not a view.
          for (const input of [null, {}, new ArrayBuffer(1)]) {
            const error = thrown(() => decoder.end(input as never));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("invalid receiver and write property", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const end = StringDecoder.prototype.end;
          for (const receiver of [null, {}, { write: 1 }]) {
            for (const input of [undefined, "", new Uint8Array()]) {
              const error = thrown(() => end.call(receiver as never, input));
              expect(error).toHaveExactConstructor(TypeError);
            }
          }
          const error = thrown(() => end.call({ write: StringDecoder.prototype.write } as never, new Uint8Array()));
          expect(error).toHaveExactConstructor(TypeError);
        });
      });

      describe("diagnostic formatting", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const decoder = new StringDecoder();
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          for (const input of [proxy, { constructor: 1 }, { constructor: { name: Symbol() } }]) {
            const error = thrown(() => decoder.end(input as never));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("native UTF-8 string length limit", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          // Isolate large native allocations; the oversized UTF-8 branch rejects
          // before scanning/copying the body. Allocation is outside the catch.
          const output = runNode(["--input-type=module", "-e", `
            import assert from 'node:assert/strict';
            import { StringDecoder } from 'node:string_decoder';
            import { constants } from 'node:buffer';
            const input = new Uint8Array(constants.MAX_STRING_LENGTH + 1);
            const decoder = new StringDecoder('utf8');
            let caught = false;
            try {
              try { decoder.end(input); }
              catch (error) {
                caught = true;
                assert.equal(error.constructor, Error);
              }
              assert.equal(caught, true);
              console.log('native-limit-ok');
            } finally { input.buffer.transfer(0); }
          `], { timeout: 20_000 });
          expect(output.trim()).toBe("native-limit-ok");
        });
      });

      describe("flush concatenation length limit", () => {
        it("throws RangeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const { constants } = await import("node:buffer");
          const decoder = new StringDecoder();
          decoder.write(new Uint8Array([0xe2]));
          const input = "x".repeat(constants.MAX_STRING_LENGTH);
          const error = thrown(() => decoder.end(input));
          expect(error).toHaveExactConstructor(RangeError);
          expect(decoder.end()).toBe("");
        });
      });

      describe("uninitialized namespace constructor export", () => {
        it("throws ReferenceError", { tags: ["node-only"] }, async () => {
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const { pathToFileURL } = await import("node:url");
          const { runInThisContext, constants } = await import("node:vm");
          const directory = await mkdtemp(join(tmpdir(), "string-decoder-end-"));
          try {
            const file = join(directory, "fixture.mjs");
            await writeFile(file, `
              import * as namespace from './fixture.mjs';
              import { StringDecoder } from 'node:string_decoder';
              const decoder = new StringDecoder();
              const limit = Object.getOwnPropertyDescriptor(Error, 'stackTraceLimit');
              let captured;
              try { decoder.end(namespace); }
              catch (error) { captured = error; }
              finally { Object.defineProperty(Error, 'stackTraceLimit', limit); }
              export { captured };
              export let constructor;
            `);
            const { captured } = await runInThisContext(`import(${JSON.stringify(pathToFileURL(file).href)})`, {
              importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
            });
            expect(captured).toHaveExactConstructor(ReferenceError);
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });
      });

      describe("custom inspection return cycle", () => {
        it("throws RangeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const { inspect } = await import("node:util");
          const first = { __proto__: null, [inspect.custom](): object { return second; } };
          const second = { [inspect.custom](): object { return first; } };
          const decoder = new StringDecoder();
          const error = thrown(() => decoder.end(first as never));
          expect(error).toHaveExactConstructor(RangeError);
        });
      });

      describe("namespace formatting through a function name", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const { inspect } = await import("node:util");
          const { runInThisContext, constants } = await import("node:vm");
          const url = 'data:text/javascript,export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
          const namespace = await runInThisContext(`import(${JSON.stringify(url)})`, {
            importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
          });
          const decoder = new StringDecoder();
          const direct = { __proto__: null, [inspect.custom]() { return namespace; } };
          const control = thrown(() => decoder.end(direct as never));
          expect(control).toHaveExactConstructor(TypeError);
          const named = function () {};
          Object.defineProperty(named, "name", { value: namespace });
          const input = { __proto__: null, [inspect.custom]() { return named; } };
          const error = thrown(() => decoder.end(input as never));
          expect(error).toHaveExactConstructor(Error);
          expect(error).not.toBe(17);
        });
      });

      for (const [dependency, makeInput] of invalidArgumentEffects) {
        describe(`invalid buf diagnostic: ${dependency}`, () => {
          it("propagates supplied effect", { tags: ["node-only"] }, async () => {
            const { StringDecoder } = await import("node:string_decoder");
            const decoder = new StringDecoder();
            const sentinel = { dependency };
            expect(thrown(() => decoder.end(makeInput(sentinel) as never))).toBe(sentinel);
          });
        });
      }

      describe("declared buffer-view shape without native view brand", () => {
        it("propagates supplied effect", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const decoder = new StringDecoder();
          const sentinel = { effect: "proxied view constructor read" };
          const input = throwOnGet(new Uint8Array(), "constructor", sentinel);
          expect(thrown(() => decoder.end(input))).toBe(sentinel);
        });
      });

      describe("this.write get/call when buf is supplied; native state read always", () => {
        it("propagates supplied effect", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const decoder = new StringDecoder();
          const end = StringDecoder.prototype.end;
          const sentinel = { effect: "receiver" };
          const receiver = throwOnGet(decoder, "write", sentinel);
          expect(thrown(() => end.call(receiver, "x"))).toBe(sentinel);
          expect(end.call(receiver)).toBe(""); // Omitted buf avoids the write getter.
          expect(thrown(() => end.call({ write() { throw sentinel; } } as never, "x"))).toBe(sentinel);
          const state = Object.getOwnPropertySymbols(decoder).find(key => key.description === "kNativeDecoder");
          expect(state).toBeDefined();
          for (const input of [undefined, "x"]) {
            expect(thrown(() => end.call(throwOnGet(decoder, state!, sentinel), input))).toBe(sentinel);
          }
        });
      });

      describe("supplied write result conversion on flush", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const decoder = new StringDecoder();
          decoder.write(new Uint8Array([0xe2]));
          const receiver = Object.create(decoder, { write: { value() { return Symbol(); } } });
          const error = thrown(() => StringDecoder.prototype.end.call(receiver, ""));
          expect(error).toHaveExactConstructor(TypeError);
        });

        it("throws non-Error value", { tags: ["node-only"] }, async () => {
          const { StringDecoder } = await import("node:string_decoder");
          const sentinel = { marker: "conversion" };
          const decoder = new StringDecoder();
          decoder.write(new Uint8Array([0xe2]));
          const receiver = Object.create(decoder, {
            write: { value() { return { [Symbol.toPrimitive]() { throw sentinel; } }; } },
          });
          expect(thrown(() => StringDecoder.prototype.end.call(receiver, ""))).toBe(sentinel);
        });
      });

      it("avoids write-result coercion without buffered bytes", { tags: ["node-only"] }, async () => {
        const { StringDecoder } = await import("node:string_decoder");
        const decoder = new StringDecoder();
        const result = { [Symbol.toPrimitive]() { throw new Error("must not coerce without flush"); } };
        const receiver = Object.create(decoder, { write: { value: () => result } });
        expect(StringDecoder.prototype.end.call(receiver, "")).toBe(result);
      });

      it("handles detached and out-of-bounds views in an isolated native process", { tags: ["node-only"] }, async () => {
        const output = runNode(["--input-type=module", "-e", `
          import assert from 'node:assert/strict';
          import { StringDecoder } from 'node:string_decoder';
          const buffer = new ArrayBuffer(4, { maxByteLength: 8 });
          const views = [new Uint8Array(buffer, 2, 2), new DataView(buffer, 2, 2)];
          buffer.resize(0);
          for (const view of views) assert.equal(new StringDecoder().end(view), '');
          buffer.transfer(0);
          for (const view of views) assert.equal(new StringDecoder().end(view), '');
          assert.equal(new StringDecoder().end(new DataView(new Uint8Array([97]).buffer)), 'a');
          assert.equal(new StringDecoder().end(new Uint8Array(new SharedArrayBuffer(1))), '\\0');
          console.log('native-views-ok');
        `], { timeout: 20_000 });
        expect(output.trim()).toBe("native-views-ok");
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("write", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });
});
