// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, expectThrown, throwOnGet } from "../../support/errors";
import { Console } from "node:console";
import { inspect } from "node:util";
import { describe, expect, it } from "vitest";

describe("node:console", () => {
  describe("global", () => {
    describe("Console", () => {
      describe("Console", () => {
        describe("new", () => {
          // Console accepts structural streams; construction must not call write.
          const stdout = { write() { throw new Error("unexpected write"); } };
          const construct = (options: unknown) => Reflect.construct(Console, [options]);
          const caught = captureThrown;
          const check = (call: () => unknown, ctor: ErrorConstructor) => {
            expectThrown(call, ctor);
          };

          for (const [scenario, options] of [
            ["missing stdout", undefined],
            ["non-writable stdout", { stdout: {} }],
            ["non-writable stderr", { stdout, stderr: {} }],
            ["invalid color mode", { stdout, colorMode: "invalid" }],
            ["non-number indentation", { stdout, groupIndentation: "2" }],
            ["null inspect options", { stdout, inspectOptions: null }],
            ["array inspect options", { stdout, inspectOptions: [] }],
            ["function inspect options", { stdout, inspectOptions() {} }],
            ["conflicting colors", { stdout, colorMode: false, inspectOptions: { colors: false } }],
            ["conflicting map colors", { stdout, colorMode: "auto", inspectOptions: new Map([[stdout, { colors: true }]]) }],
            ["null map value", { stdout, inspectOptions: new Map([[stdout, null]]) }],
            ["diagnostic invalid constructor", { stdout, groupIndentation: { constructor: 1 } }],
          ] as const) {
            describe(scenario, () => {
              it("throws TypeError", () => check(() => construct(options), TypeError));
            });
          }

          describe("legacy overload stderr", () => {
            it("throws TypeError", () => {
              check(() => Reflect.construct(Console, [stdout, {}]), TypeError);
            });
          });

          for (const groupIndentation of [-1, 1001, 0.5, NaN, Infinity]) {
            describe(`indentation ${groupIndentation}`, () => {
              it("throws RangeError", () => {
                check(() => construct({ stdout, groupIndentation }), RangeError);
              });
            });
          }

          describe("revoked proxies", () => {
            it("throws TypeError", () => {
              const { proxy, revoke } = Proxy.revocable({}, {});
              revoke();
              check(() => construct(proxy), TypeError);
              check(() => construct({ stdout, inspectOptions: proxy }), TypeError);
            });
          });

          describe("subclass non-callable override and read-only method", () => {
            it("throws TypeError", () => {
              for (const descriptor of [{ value: null }, { value() {}, writable: false }]) {
                function Derived() {}
                Derived.prototype = Object.create(Console.prototype);
                Object.defineProperty(Derived.prototype, "log", descriptor);
                check(() => Reflect.construct(Console, [{ stdout }], Derived), TypeError);
              }
              // A different newTarget lacking Console's methods is invalid too.
              check(() => Reflect.construct(Console, [{ stdout }], function Other() {}), TypeError);
            });
          });

          describe("diagnostic custom inspection", () => {
            it("propagates supplied effect", () => {
              const sentinel = { effect: "colorMode[inspect.custom] call" };
              const colorMode = { [inspect.custom]() { throw sentinel; } };
              expect(caught(() => construct({ stdout, colorMode }))).toBe(sentinel);
            });
            it("throws RangeError", () => {
              // Both hooks return normally; inspect's recursive return traversal fails.
              const first = { [inspect.custom](): object { return second; } };
              const second = { [inspect.custom](): object { return first; } };
              const stackLimit = Object.getOwnPropertyDescriptor(Error, "stackTraceLimit");
              const error = caught(() => construct({ stdout, colorMode: first })) as Error;
              expect(error).toHaveExactConstructor(RangeError);
              expect(Object.getOwnPropertyDescriptor(Error, "stackTraceLimit")).toEqual(stackLimit);
            });
          });

          describe("diagnostic namespace formatting", () => {
            it("throws Error", { tags: ["node-only"] }, async () => {
              const { runInThisContext, constants } = await import("node:vm");
              const url = 'data:text/javascript,export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
              const colorMode = await runInThisContext(`import(${JSON.stringify(url)})`, {
                importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
              });
              const stackLimit = Object.getOwnPropertyDescriptor(Error, "stackTraceLimit");
              // Namespace formatting replaces 17 with a newly created internal error.
              check(() => construct({ stdout, colorMode }), Error);
              expect(Object.getOwnPropertyDescriptor(Error, "stackTraceLimit")).toEqual(stackLimit);
            });
          });

          describe("uninitialized native namespace bindings", () => {
            it("throws ReferenceError", { tags: ["node-only"] }, async () => {
              const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
              const { tmpdir } = await import("node:os");
              const { join } = await import("node:path");
              const { pathToFileURL } = await import("node:url");
              const { runInThisContext, constants } = await import("node:vm");
              const directory = await mkdtemp(join(tmpdir(), "console-new-"));
              try {
                for (const [binding, argument] of [
                  ["stdout", "namespace"], // options destructuring
                  ["write", "namespace"], // positional/options discrimination
                  ["write", "{ stdout: namespace }"], // stream validation
                  ["colors", "{ stdout: sink, inspectOptions: namespace }"],
                  ["constructor", "{ stdout: sink, groupIndentation: namespace }"],
                ]) {
                  const file = join(directory, `${binding}-${argument.length}.mjs`);
                  const url = pathToFileURL(file).href;
                  await writeFile(file, `
                    import * as namespace from ${JSON.stringify(url)};
                    import { Console } from 'node:console';
                    const sink = { write() {} };
                    const stackLimit = Object.getOwnPropertyDescriptor(Error, 'stackTraceLimit');
                    let captured;
                    try { new Console(${argument}); } catch (error) { captured = error; }
                    finally {
                      if (stackLimit) Object.defineProperty(Error, 'stackTraceLimit', stackLimit);
                      else Reflect.deleteProperty(Error, 'stackTraceLimit');
                    }
                    export { captured };
                    export let ${binding};
                  `);
                  // Bypass Vitest's transformed namespaces; import itself succeeds.
                  const { captured } = await runInThisContext(`import(${JSON.stringify(url)})`, {
                    importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
                  });
                  expect(captured).toHaveExactConstructor(ReferenceError);
                }
              } finally {
                await rm(directory, { recursive: true, force: true });
              }
            });
          });

          describe("diagnostic built-in formatting", () => {
            it("throws TypeError", () => {
              const colorMode = new Uint8Array(1);
              structuredClone(colorMode.buffer, { transfer: [colorMode.buffer] });
              Object.setPrototypeOf(colorMode, null);
              check(() => construct({ stdout, colorMode }), TypeError);
              const set = new Set([1]);
              Object.defineProperty(set, Symbol.iterator, { value: () => ({ next: () => 1 }) });
              check(() => construct({ stdout, colorMode: set }), TypeError);
            });
            it("throws non-Error value", () => {
              const supplied = Symbol("diagnostic iterator");
              const colorMode = new Set([1]);
              Object.defineProperty(colorMode, Symbol.iterator, { value() { throw supplied; } });
              expect(caught(() => construct({ stdout, colorMode }))).toBe(supplied);
              const hookGetter = { get [inspect.custom]() { throw supplied; } };
              expect(caught(() => construct({ stdout, colorMode: hookGetter }))).toBe(supplied);
            });
          });

          describe("property, proxy, diagnostic and subclass propagation", () => {
            it("throws non-Error value", () => {
              const value = { sentinel: true };
              const fail = () => { throw value; };
              for (const key of ["write", "stdout", "stderr", "ignoreErrors", "colorMode", "inspectOptions", "groupIndentation"]) {
                const options = { stdout };
                Object.defineProperty(options, key, { get: fail });
                expect(caught(() => construct(options))).toBe(value);
              }
              expect(caught(() => construct(throwOnGet({ stdout }, "write", value)))).toBe(value);
              expect(caught(() => construct({ stdout: { get write() { return fail(); } } }))).toBe(value);
              const inspectOptions = { get colors() { return fail(); } };
              expect(caught(() => construct({ stdout, inspectOptions }))).toBe(value);
              expect(caught(() => construct({ stdout, inspectOptions: new Map([[stdout, inspectOptions]]) }))).toBe(value);
              // ERR_INVALID_ARG_TYPE formats the supplied value before throwing.
              expect(caught(() => construct({ stdout, groupIndentation: { get constructor() { return fail(); } } }))).toBe(value);
              // The second colorMode read occurs only after inspectOptions.colors.
              let reads = 0;
              expect(caught(() => construct({ stdout, inspectOptions: { colors: true }, get colorMode() { return reads++ ? fail() : "auto"; } }))).toBe(value);
              for (const descriptor of [
                { get: fail },
                { get: () => function log() {}, set: fail },
                // Function.prototype.bind reads the overridden method's length and name.
                ...["length", "name"].map(key => ({ value: throwOnGet(function log() {}, key, value), writable: true })),
              ]) {
                function Derived() {}
                Derived.prototype = Object.create(Console.prototype);
                Object.defineProperty(Derived.prototype, "log", descriptor);
                expect(caught(() => Reflect.construct(Console, [{ stdout }], Derived))).toBe(value);
              }
              const NewTarget = throwOnGet(function Derived() {}, "prototype", value);
              expect(caught(() => Reflect.construct(Console, [{ stdout }], NewTarget))).toBe(value);
              for (const key of Reflect.ownKeys(Console.prototype)) {
                if (typeof key !== "symbol" || !["kBindStreamsEager", "kBindProperties"].includes(key.description!)) continue;
                function Derived() {}
                Derived.prototype = Object.create(Console.prototype);
                Object.defineProperty(Derived.prototype, key, { value: fail });
                expect(caught(() => Reflect.construct(Console, [{ stdout }], Derived))).toBe(value);
              }
            });
          });

          it("constructs without writing or using a call receiver", () => {
            for (const groupIndentation of [0, 1000]) {
              expect(construct({ stdout, groupIndentation, ignoreErrors: false })).toBeInstanceOf(Console);
            }
            expect(Reflect.construct(Console, [stdout, undefined, false])).toBeInstanceOf(Console);
            expect(Reflect.apply(Console, null, [{ stdout }])).toBeInstanceOf(Console);
            expect(Reflect.apply(Console, Object.freeze({}), [{ stdout }])).toBeInstanceOf(Console);
            // Native Map iteration ignores a supplied iterator override.
            const inspectOptions = new Map([[stdout, {}]]);
            Object.defineProperty(inspectOptions, Symbol.iterator, { get() { throw new Error("unused iterator"); } });
            expect(construct({ stdout, inspectOptions })).toBeInstanceOf(Console);
          });
        });
      });

      describe("assert", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("clear", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("count", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("countReset", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("debug", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("dir", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("dirxml", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("error", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("group", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("groupCollapsed", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("groupEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("info", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("log", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("profile", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("profileEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("table", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("time", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeLog", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeStamp", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("trace", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("warn", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("console", () => {
      describe("ConsoleConstructor", () => {
        describe("new", () => {
          it("TODO", () => {
            // TODO
          });
        });
      });

      describe("assert", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("clear", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("count", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("countReset", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("debug", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("dir", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("dirxml", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("error", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("group", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("groupCollapsed", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("groupEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("info", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("log", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("profile", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("profileEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("table", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("time", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeEnd", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeLog", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("timeStamp", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("trace", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("warn", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });
  });
});
