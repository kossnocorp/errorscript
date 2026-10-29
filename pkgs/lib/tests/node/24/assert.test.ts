// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { Assert, AssertionError } from "node:assert";
import { inspect } from "node:util";
import { describe, expect, it } from "vitest";
import { captureThrown, expectThrown, throwOnGet, withPropertyRestored } from "../../support/errors";

describe("node:assert", () => {
  describe("assert", () => {
    it("TODO", () => {
      // TODO
    });
    describe("Assert", () => {
      describe("new", () => {
        const capture = captureThrown;
        const expectTypeError = (fn: () => unknown) => {
          expectThrown(fn, TypeError);
        };
        // Node's diagnostic error constructor does not restore stackTraceLimit
        // if inspecting the invalid value throws. Keep that leak test-local.
        const withStackLimitRestored = (fn: () => void) => {
          return withPropertyRestored(Error, "stackTraceLimit", fn);
        };

        it("throws TypeError", { tags: ["node-only"] }, () => {
          for (const diff of ["invalid"]) {
            expectTypeError(
              () => new Assert({ diff } as never),
            );
          }
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          expectTypeError(
            () => Reflect.apply(Assert, {}, []),
          );
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          expectTypeError(() => new Assert(proxy));
          const duplicateKeys = new Proxy({}, { ownKeys: () => ["x", "x"] });
          expectTypeError(() => new Assert(duplicateKeys));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          function NewTarget() {}
          Object.defineProperty(NewTarget.prototype, "AssertionError", {
            value: undefined,
            writable: false,
          });
          expectTypeError(() => Reflect.construct(Assert, [], NewTarget));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          function NewTarget() {}
          Object.defineProperty(NewTarget.prototype, "AssertionError", {
            set(this: object) {
              Object.preventExtensions(this);
            },
          });
          expectTypeError(() => Reflect.construct(Assert, [], NewTarget));
        });

        it("throws Error", { tags: ["node-only"] }, () => {
          const supplied = new Error("options getter");
          const options = {
            get unrelated() {
              throw supplied;
            },
          };
          expect(capture(() => new Assert(options as never))).toBe(supplied);

        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = { marker: "proxy trap" };
          for (const trap of ["ownKeys", "getOwnPropertyDescriptor", "get"]) {
            const options = new Proxy({ strict: true }, {
              [trap](_target: object, key?: PropertyKey) {
                // Object spread enumerates options, then describes/reads each key.
                if (trap !== "ownKeys") expect(key).toBe("strict");
                throw supplied;
              },
            });
            expect(capture(() => new Assert(options))).toBe(supplied);
          }
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("construction hooks");
          function NewTarget() {}
          const proxiedTarget = throwOnGet(NewTarget, "prototype", supplied);
          expect(capture(() => Reflect.construct(Assert, [], proxiedTarget))).toBe(supplied);
          for (const property of ["AssertionError", "strictEqual"]) {
            function Target() {}
            Object.defineProperty(Target.prototype, property,
              property === "AssertionError"
                ? { set() { throw supplied; } }
                : { get() { throw supplied; } });
            expect(capture(() => Reflect.construct(Assert, [], Target))).toBe(supplied);
          }
        });

        it("throws SyntaxError", { tags: ["node-only"] }, () => {
          const supplied = new SyntaxError("custom inspection");
          const diff = { [inspect.custom]() { throw supplied; } };
          withStackLimitRestored(() => {
            expect(capture(() => new Assert({ diff } as never))).toBe(supplied);

          });
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = 17;
          const diff = { get [inspect.custom]() { throw supplied; } };
          withStackLimitRestored(() => {
            expect(capture(() => new Assert({ diff } as never))).toBe(supplied);
          });
        });

        it("throws RangeError", { tags: ["node-only"] }, () => {
          // Hooks return normally; inspect itself recurses between their results.
          const first = { [inspect.custom](): object { return second; } };
          const second = { [inspect.custom](): object { return first; } };
          withStackLimitRestored(() => {
            const error = capture(() => new Assert({ diff: first } as never));
            expect(error).toHaveExactConstructor(RangeError);
          });
        });

        it("throws ReferenceError", { tags: ["node-only"] }, async () => {
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const { pathToFileURL } = await import("node:url");
          const { runInThisContext, constants } = await import("node:vm");
          const directory = await mkdtemp(join(tmpdir(), "assert-new-"));
          try {
            const file = join(directory, "fixture.mjs");
            await writeFile(file, `
              import * as options from './fixture.mjs';
              import { Assert } from 'node:assert';
              let captured;
              try { new Assert(options); } catch (error) { captured = error; }
              export { captured };
              export let uninitialized;
            `);
            const url = pathToFileURL(file).href;
            // Use Node's native ESM loader, not Vitest's transformed namespace.
            const { captured } = await runInThisContext(`import(${JSON.stringify(url)})`, {
              importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
            });
            expect(captured).toHaveExactConstructor(ReferenceError);
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });

        it("throws Error", { tags: ["node-only"] }, async () => {
          // A real module namespace forces inspect's formatNamespaceObject path.
          const url = 'data:text/javascript,export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
          const namespace = await import(/* @vite-ignore */ url);
          withStackLimitRestored(() => {
            const error = capture(() => new Assert({ diff: namespace } as never));
            expect(error).toHaveExactConstructor(Error);
          });
        });
      });
    });

    describe("AssertPredicate", () => {
      it("TODO", () => {
        // TODO
      });
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("AssertionError", () => {
      describe("new", () => {
        const capture = captureThrown;
        const expectTypeError = (fn: () => unknown) => {
          expectThrown(fn, TypeError);
        };

        it("throws TypeError", { tags: ["node-only"] }, () => {
          for (const options of [undefined, null, [], () => {}, 0]) {
            expectTypeError(() => new AssertionError(options as never));
          }
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          expectTypeError(() => Reflect.apply(AssertionError, {}, [{}]));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          expectTypeError(() => new AssertionError(proxy));
          const target = Object.defineProperty({}, "message", { value: "fixed" });
          const options = new Proxy(target, { get: () => "different" });
          expectTypeError(() => new AssertionError(options));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          for (const message of [Object.create(null), { [Symbol.toPrimitive]: 0 },
            { [Symbol.toPrimitive]: () => Symbol() }, { toString: () => ({}), valueOf: () => ({}) }]) {
            expectTypeError(() => new AssertionError({ message } as never));
          }
          // String(Symbol) is allowed, but diff interpolation uses ToString(Symbol).
          expectTypeError(() => new AssertionError({ message: Symbol(), operator: "strictEqual" } as never));
          expectTypeError(() => new AssertionError({ operator: Symbol() } as never));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          for (const details of [[null], { length: Symbol() }]) {
            expectTypeError(() => new AssertionError({ message: "details", details } as never));
          }
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          // inspect reconstructs null-prototype typed arrays, rejecting detached buffers.
          const actual = new Uint8Array(1);
          structuredClone(actual.buffer, { transfer: [actual.buffer] });
          Object.setPrototypeOf(actual, null);
          expectTypeError(() => new AssertionError({ actual }));
          const set = new Set([1]);
          Object.defineProperty(set, Symbol.iterator, { value: () => ({ next: () => 1 }) });
          expectTypeError(() => new AssertionError({ actual: set }));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          function Target() {}
          Object.defineProperty(Target.prototype, "generatedMessage", { value: false });
          expectTypeError(() => Reflect.construct(AssertionError, [{ message: "x" }], Target));
          function NonExtensibleTarget() {}
          Object.defineProperty(NonExtensibleTarget.prototype, "generatedMessage", {
            set(this: object) { Object.preventExtensions(this); },
          });
          expectTypeError(() => Reflect.construct(AssertionError, [{ message: "x" }], NonExtensibleTarget));
        });

        it("throws TypeError", { tags: ["node-only"] }, () => {
          function Target() {}
          Object.defineProperty(Target.prototype, "operator", {
            set(this: object) { Object.freeze(this); },
          });
          // Earlier writes succeed; Error.captureStackTrace cannot replace frozen stack.
          expectTypeError(() => Reflect.construct(AssertionError, [{ message: "x" }], Target));
        });

        it("throws RangeError", { tags: ["node-only"] }, () => {
          const actual = new Error("cyclic cause");
          actual.cause = actual;
          const expected = new Error("other");
          const error = capture(() => new AssertionError({ actual, expected }));
          expect(error).toHaveExactConstructor(RangeError);
        });

        it("throws RangeError", { tags: ["node-only"] }, () => {
          // Unlike actual/expected formatting, invalid-options diagnostics allow custom inspection.
          const first = { [inspect.custom](): object { return second; } };
          const second = { [inspect.custom](): object { return first; } };
          const options: unknown[] = [];
          Object.defineProperties(options, {
            constructor: { value: null },
            [inspect.custom]: { value: () => first },
          });
          const error = capture(() => new AssertionError(options as never));
          expect(error).toHaveExactConstructor(RangeError);
        });

        it("throws ReferenceError", { tags: ["node-only"] }, async () => {
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const { pathToFileURL } = await import("node:url");
          const { runInThisContext, constants } = await import("node:vm");
          const directory = await mkdtemp(join(tmpdir(), "assertion-error-new-"));
          try {
            const file = join(directory, "fixture.mjs");
            await writeFile(file, `
              import * as options from './fixture.mjs';
              import { AssertionError } from 'node:assert';
              let captured;
              try { new AssertionError(options); } catch (error) { captured = error; }
              export { captured };
              export let message;
            `);
            const url = pathToFileURL(file).href;
            const { captured } = await runInThisContext(`import(${JSON.stringify(url)})`, {
              importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
            });
            expect(captured).toHaveExactConstructor(ReferenceError);
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });

        it("throws Error", { tags: ["node-only"] }, async () => {
          // customInspect is disabled, but constructor-name access remains observable.
          const source = `
            const C = function() {};
            Object.defineProperty(C, "name", { get() { throw 17; } });
            export const value = { constructor: C };
          `;
          const url = `data:text/javascript,${encodeURIComponent(source)}`;
          const actual = await import(/* @vite-ignore */ url);
          const error = capture(() => new AssertionError({ actual }));
          expect(error).toHaveExactConstructor(Error);
        });

        it("throws SyntaxError", { tags: ["node-only"] }, () => {
          const supplied = new SyntaxError("message conversion");
          const message = { [Symbol.toPrimitive]() { throw supplied; } };
          expect(capture(() => new AssertionError({ message } as never))).toBe(supplied);

        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("options access");
          for (const property of ["message", "operator", "stackStartFn", "details",
            "stackStartFunction", "diff", "actual", "expected"]) {
            const options = Object.defineProperty({}, property, { get() { throw supplied; } });
            expect(capture(() => new AssertionError(options))).toBe(supplied);
          }
          const options = throwOnGet({}, "message", supplied);
          expect(capture(() => new AssertionError(options))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = { marker: "invalid array diagnostic" };
          const options: unknown[] = [];
          Object.defineProperty(options, "constructor", { get() { throw supplied; } });
          expect(capture(() => new AssertionError(options as never))).toBe(supplied);
          const inspected: unknown[] = [];
          Object.defineProperties(inspected, {
            constructor: { value: null },
            [inspect.custom]: { get() { throw supplied; } },
          });
          expect(capture(() => new AssertionError(inspected as never))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = 17;
          const expected = new Error("other");
          for (const property of ["message", "cause", "extra"]) {
            const actual = new Error("copy");
            Object.defineProperty(actual, property, { enumerable: true, get() { throw supplied; } });
            expect(capture(() => new AssertionError({ actual, expected }))).toBe(supplied);
          }
          for (const [trap, property] of [
            ["has", "stack"], ["getPrototypeOf", undefined], ["ownKeys", undefined],
            ["getOwnPropertyDescriptor", "extra"], ["get", "message"],
          ] as const) {
            const target = Object.assign(new Error("proxy"), { extra: 1 });
            const actual = new Proxy(target, {
              [trap](object: object, key?: PropertyKey, receiver?: unknown) {
                if (key === property) throw supplied;
                // Only keyed traps can reach here; forward earlier copy/brand operations.
                if (trap === "has") return Reflect.has(object, key!);
                if (trap === "getOwnPropertyDescriptor") return Reflect.getOwnPropertyDescriptor(object, key!);
                return Reflect.get(object, key!, receiver);
              },
            });
            expect(capture(() => new AssertionError({ actual, expected }))).toBe(supplied);
          }
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("operator coercion");
          const operator = { [Symbol.toPrimitive]() { throw supplied; } };
          expect(capture(() => new AssertionError({ operator } as never))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = { marker: "inspection" };
          function Constructor() {}
          Object.defineProperty(Constructor, "name", { get() { throw supplied; } });
          const actual = { constructor: Constructor };
          expect(capture(() => new AssertionError({ actual }))).toBe(supplied);
          // Inspection evaluates getters and recursively formats their thrown values.
          const getter = { get value() { throw actual; } };
          expect(capture(() => new AssertionError({ actual: getter }))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("iterator next");
          for (const actual of [new Set([1]), new Map([[1, 2]])]) {
            Object.defineProperty(actual, Symbol.iterator, {
              value: () => ({ next() { throw supplied; } }),
            });
            expect(capture(() => new AssertionError({ actual }))).toBe(supplied);
          }
          const actual = new Set([1]);
          Object.defineProperty(actual, Symbol.iterator, {
            value: () => ({
              next: () => ({ value: 1, done: false }),
              return() { throw supplied; },
            }),
          });
          // Inspection stops at the original size and closes the supplied iterator.
          expect(capture(() => new AssertionError({ actual }))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("details");
          for (const property of ["message", "actual", "expected", "operator", "stack"]) {
            const entry = Object.defineProperty({}, property, { get() { throw supplied; } });
            expect(capture(() => new AssertionError({ message: "x", details: [entry] } as never))).toBe(supplied);
          }
          for (const details of [
            { get length() { throw supplied; } },
            { length: { [Symbol.toPrimitive]() { throw supplied; } } },
            { length: 1, get 0() { throw supplied; } },
          ]) {
            expect(capture(() => new AssertionError({ message: "x", details } as never))).toBe(supplied);
          }
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("construction hooks");
          function Target() {}
          const target = throwOnGet(Target, "prototype", supplied);
          expect(capture(() => Reflect.construct(AssertionError, [{ message: "x" }], target))).toBe(supplied);
          Object.defineProperty(Target.prototype, "generatedMessage", { set() { throw supplied; } });
          expect(capture(() => Reflect.construct(AssertionError, [{ message: "x" }], Target))).toBe(supplied);
        });

        it("throws non-Error value", { tags: ["node-only"] }, () => {
          const supplied = Symbol("stack formatting");
          const descriptor = Object.getOwnPropertyDescriptor(Error, "prepareStackTrace");
          let captured: unknown;
          try {
            Object.defineProperty(Error, "prepareStackTrace", {
              configurable: true, writable: true, value() { throw supplied; },
            });
            captured = capture(() => new AssertionError({ message: "x" }));
          } finally {
            if (descriptor) Object.defineProperty(Error, "prepareStackTrace", descriptor);
            else Reflect.deleteProperty(Error, "prepareStackTrace");
          }
          expect(captured).toBe(supplied);
        });

        it("returns an AssertionError without throwing it", { tags: ["node-only"] }, () => {
          const actual = {
            [inspect.custom]() { throw new Error("must not call custom inspector"); },
            get value() { throw 17; },
          };
          for (const operator of [undefined, "strictEqual", "deepStrictEqual", "partialDeepStrictEqual",
            "notStrictEqual", "notDeepStrictEqual", "deepEqual", "notDeepEqual", "unknown"]) {
            const error = new AssertionError({ actual, expected: {}, operator });
            expect(error).toHaveExactConstructor(AssertionError);
          }
          expect(new AssertionError({ message: Symbol("allowed") } as never).message).toBe("Symbol(allowed)");
        });
      });
    });

    describe("CallTracker", () => {
      describe("calls", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("getCalls", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("report", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("reset", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("verify", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("deepEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("deepStrictEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("doesNotMatch", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("doesNotReject", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("doesNotThrow", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("equal", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("fail", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("ifError", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("match", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("notDeepEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("notDeepStrictEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("notEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("notStrictEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("ok", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("partialDeepStrictEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("rejects", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("strictEqual", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("throws", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });
});
