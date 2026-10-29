// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, throwOnGet } from "../../support/errors";
import { runNode } from "../../support/node";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { URL } from "node:url";
import { describe, expect, it } from "vitest";

describe("node:url", () => {
  describe("URL", () => {
    describe("new", () => {
      const thrownBy = captureThrown;

      it("throws TypeError", () => {
        const error = thrownBy(() => Reflect.construct(URL, []));
        expect(error).toHaveExactConstructor(TypeError);
      });

      it("throws TypeError", () => {
        // Missing scheme/base, invalid host/IPv4/IPv6/port, and file authority.
        const inputs = [
          undefined,
          "relative/path",
          "http://",
          "http://exa mple.test/",
          "http://256.0.0.1/",
          "http://[::1",
          "http://example.test:65536/",
          "file://host:80/path",
        ];
        for (const input of inputs) {
          const error = thrownBy(() => Reflect.construct(URL, [input]));
          expect(error).toHaveExactConstructor(TypeError);
        }
      });

      it("throws TypeError", () => {
        // Even an absolute input does not bypass invalid-base validation.
        for (const [input, base] of [
          ["https://example.test/", "invalid base"],
          ["relative", "data:text/plain,opaque"],
        ]) {
          const error = thrownBy(() => new URL(input, base));
          expect(error).toHaveExactConstructor(TypeError);
        }
        // A public third argument cannot supply Node's private parse token.
        const error = thrownBy(() =>
          Reflect.construct(URL, ["invalid", undefined, Symbol("kParseURL")]),
        );
        expect(error).toHaveExactConstructor(TypeError);
      });

      it("throws TypeError", () => {
        // Both argument conversions use ToString, not String(symbol).
        for (const args of [[Symbol("input")], ["https://example.test/", Symbol("base")]]) {
          const error = thrownBy(() => Reflect.construct(URL, args));
          expect(error).toHaveExactConstructor(TypeError);
        }
      });

      it("throws TypeError", () => {
        for (const input of [
          { [Symbol.toPrimitive]: 1 },
          { [Symbol.toPrimitive]: () => ({}) },
          { toString: () => ({}), valueOf: () => ({}) },
        ]) {
          const error = thrownBy(() => Reflect.construct(URL, [input]));
          expect(error).toHaveExactConstructor(TypeError);
        }
      });

      it("throws TypeError", () => {
        const input = Proxy.revocable({}, {});
        const newTarget = Proxy.revocable(function NewTarget() {}, {});
        const invariantViolation = new Proxy(
          Object.defineProperty({}, "toString", { value: () => "https://example.test/" }),
          {
            get(target, key, receiver) {
              if (key === "toString") return () => "https://different.test/";
              return Reflect.get(target, key, receiver);
            },
          },
        );
        input.revoke();
        newTarget.revoke();
        for (const action of [
          () => Reflect.construct(URL, [input.proxy]),
          () => Reflect.construct(URL, ["https://example.test/", input.proxy]),
          () => Reflect.construct(URL, ["https://example.test/"], newTarget.proxy),
          () => Reflect.construct(URL, [invariantViolation]),
          () => Reflect.construct(URL, ["https://example.test/", invariantViolation]),
          () => Reflect.apply(URL, {}, ["https://example.test/"]),
        ]) {
          expect((thrownBy(action) as Error)).toHaveExactConstructor(TypeError);
        }
      });

      it("throws ReferenceError", { timeout: 15_000, tags: ["node-only"] }, () => {
        // Native ESM is required: Vite-transformed imports would not prove TDZ
        // semantics of a real module namespace exotic object.
        const directory = mkdtempSync(join(tmpdir(), "errs-url-namespace-"));
        try {
          const fixture = join(directory, "self.mjs");
          writeFileSync(
            fixture,
            `
            import assert from 'node:assert/strict';
            import { URL } from 'node:url';
            import * as namespace from './self.mjs';
            let checked = 0;
            function check(value) {
              // Merely passing the namespace does not read its lexical exports.
              for (const invoke of [
                () => new URL(value),
                () => new URL('https://example.test/', value),
              ]) {
                assert.throws(invoke, (error) => {
                  assert.equal(error.constructor, ReferenceError);
                  return true;
                });
                checked++;
              }
            }
            check(namespace); // toString binding is still uninitialized.
            // Ordinary prototype traversal must preserve the same exotic error.
            check(Object.create(namespace));
            // The trap itself returns normally. Proxy invariant validation reads
            // the namespace's own-property descriptor and encounters the TDZ.
            check(new Proxy(namespace, {
              get(target, key, receiver) {
                if (key === 'toString') return () => 'https://example.test/';
                return Reflect.get(target, key, receiver);
              },
            }));
            export const toString = () => ({});
            check(namespace); // OrdinaryToPrimitive now reaches valueOf's TDZ.
            export const valueOf = () => 'https://example.test/';
            assert.equal(new URL(namespace).href, 'https://example.test/');
            assert.equal(checked, 8);
            console.log('ReferenceError verified 8 branches ' + process.version);
          `,
          );
          const output = runNode([fixture]);
          expect(output.trim()).toBe(`ReferenceError verified 8 branches ${process.version}`);
        } finally {
          rmSync(directory, { recursive: true, force: true });
        }
      });

      it("throws Error", { timeout: 60_000, tags: ["node-only"] }, () => {
        // Isolate the large native parser allocation; child exit releases it.
        // Fixture creation is OUTSIDE assert.throws: only new URL is measured.
        const output = runNode(
          [
            "--input-type=module",
            "-e",
            `
          import assert from 'node:assert/strict';
          import { constants } from 'node:buffer';
          import { URL } from 'node:url';
          const input = 'https://example.test/' + '\\u0800'.repeat(Math.ceil(constants.MAX_STRING_LENGTH / 9));
          assert.throws(() => new URL(input), (error) => {
            assert.equal(error.constructor, Error);
            return true;
          });
          console.log('ERR_STRING_TOO_LONG verified ' + process.version);
        `,
          ],
          { timeout: 50_000 },
        );
        expect(output.trim()).toBe(`ERR_STRING_TOO_LONG verified ${process.version}`);
      });

      it("throws Error", () => {
        const supplied = new Error("supplied coercion failure");
        const input = {
          toString() {
            throw supplied;
          },
        };

        expect(thrownBy(() => new URL(input))).toBe(supplied);
        expect(thrownBy(() => Reflect.construct(URL, ["https://example.test/", input]))).toBe(
          supplied,
        );
      });

      it("throws SyntaxError", () => {
        // Supplied code, not URL parsing, creates this SyntaxError.
        const supplied = new SyntaxError("supplied primitive hook failure");
        const input = {
          [Symbol.toPrimitive]() {
            throw supplied;
          },
        };

        expect(thrownBy(() => Reflect.construct(URL, [input]))).toBe(supplied);
      });

      it("throws non-Error value", () => {
        const supplied = { marker: "coercion getter or proxy" };
        for (const input of [
          {
            get [Symbol.toPrimitive]() {
              throw supplied;
            },
          },
          {
            get toString() {
              throw supplied;
            },
          },
          {
            toString: () => ({}),
            get valueOf() {
              throw supplied;
            },
          },
          {
            toString: () => ({}),
            valueOf() {
              throw supplied;
            },
          },
          throwOnGet({}, Symbol.toPrimitive, supplied),
        ]) {
          expect(thrownBy(() => Reflect.construct(URL, [input]))).toBe(supplied);
          expect(thrownBy(() => Reflect.construct(URL, ["https://example.test/", input]))).toBe(
            supplied,
          );
        }
      });

      it("throws non-Error value", () => {
        const supplied = { marker: "newTarget.prototype" };
        const newTarget = new Proxy(function NewTarget() {}, {
          get(target, key, receiver) {
            if (key === "prototype") throw supplied;
            return Reflect.get(target, key, receiver);
          },
        });
        expect(thrownBy(() => Reflect.construct(URL, ["https://example.test/"], newTarget))).toBe(
          supplied,
        );
      });
    });

    describe("searchParams", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("static canParse", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static createObjectURL", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static parse", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static revokeObjectURL", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toString", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("URLPattern", () => {
    describe("exec", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("test", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("URLSearchParams", () => {
    describe("[Symbol.iterator]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("append", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("delete", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("entries", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("forEach", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("get", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getAll", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("has", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("keys", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("set", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("sort", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toString", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("values", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("URLSearchParamsIterator", () => {
    describe("[Symbol.iterator]", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("domainToASCII", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("domainToUnicode", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("fileURLToPath", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("fileURLToPathBuffer", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("format", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("parse", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("pathToFileURL", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("resolve", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("urlToHttpOptions", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
