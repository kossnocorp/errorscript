// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown } from "../../support/errors";
import { invalidArgumentEffects } from "../../support/node-diagnostics";
import { describe, expect, it } from "vitest";

describe("node:path", () => {
  describe("path", () => {
    describe("PlatformPath", () => {
      describe("basename", () => {
        for (const platform of ["posix", "win32"] as const) {
          describe(platform, () => {
            const implementation = async () => {
              const path = await import("node:path");
              return path[platform].basename as (path?: unknown, suffix?: unknown) => string;
            };
            const thrown = captureThrown;
            // Exercise both validator sites, not only the required path argument.
            const calls = (basename: Awaited<ReturnType<typeof implementation>>, value: unknown) => [
              () => basename(value),
              () => basename("file.txt", value),
            ];

            describe("argument validation and suffix-first order", () => {
              it("throws TypeError", { tags: ["node-only"] }, async () => {
                const basename = await implementation();
                // Primitive validation vs object diagnostics; other shapes have targeted repros.
                for (const value of [null, {}]) {
                  for (const call of calls(basename, value)) {
                    const error = thrown(call);
                    expect(error).toHaveExactConstructor(TypeError);
                  }
                }
                const missing = thrown(() => basename());
                expect(missing).toHaveExactConstructor(TypeError);
                const untouched = { get constructor() { expect.unreachable("path checked before suffix"); } };
                const error = thrown(() => basename(untouched, null));
                expect(error).toHaveExactConstructor(TypeError);
                expect((error as Error).message).toContain('"suffix"');
              });
            });

            describe("diagnostic operations", () => {
              it("throws TypeError", { tags: ["node-only"] }, async () => {
                const basename = await implementation();
                const { proxy, revoke } = Proxy.revocable({}, {});
                revoke();
                for (const value of [proxy, { constructor: 1 }, { constructor: { name: Symbol() } },
                  { constructor: { name: { [Symbol.toPrimitive]() { return {}; } } } }]) {
                  for (const call of calls(basename, value)) {
                    const error = thrown(call);
                    expect(error).toHaveExactConstructor(TypeError);
                  }
                }
              });
            });

            describe("uninitialized namespace constructor export", () => {
              it("throws ReferenceError", { tags: ["node-only"] }, async () => {
                const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
                const { tmpdir } = await import("node:os");
                const { join } = await import("node:path");
                const { pathToFileURL } = await import("node:url");
                const { runInThisContext, constants } = await import("node:vm");
                const directory = await mkdtemp(join(tmpdir(), "basename-"));
                try {
                  const file = join(directory, "fixture.mjs");
                  await writeFile(file, `
                    import * as namespace from './fixture.mjs';
                    import path from 'node:path';
                    export const captured = [];
                    for (const args of [[namespace], ['file.txt', namespace]]) {
                      const descriptor = Object.getOwnPropertyDescriptor(Error, 'stackTraceLimit');
                      try { path[${JSON.stringify(platform)}].basename(...args); }
                      catch (error) { captured.push(error); }
                      finally { Object.defineProperty(Error, 'stackTraceLimit', descriptor); }
                    }
                    export let constructor;
                  `);
                  const { captured } = await runInThisContext(`import(${JSON.stringify(pathToFileURL(file).href)})`, {
                    importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
                  });
                  expect(captured).toHaveLength(2);
                  for (const error of captured) {
                    expect(error).toHaveExactConstructor(ReferenceError);
                  }
                } finally {
                  await rm(directory, { recursive: true, force: true });
                }
              });
            });

            describe("custom inspection return cycle", () => {
              it("throws RangeError", { tags: ["node-only"] }, async () => {
                const basename = await implementation();
                const { inspect } = await import("node:util");
                const first = { __proto__: null, [inspect.custom](): object { return second; } };
                const second = { [inspect.custom](): object { return first; } };
                for (const call of calls(basename, first)) {
                  const error = thrown(call);
                  expect(error).toHaveExactConstructor(RangeError);
                }
              });
            });

            describe("namespace formatting through function name", () => {
              it("throws Error", { tags: ["node-only"] }, async () => {
                const basename = await implementation();
                const { inspect } = await import("node:util");
                const { runInThisContext, constants } = await import("node:vm");
                const url = 'data:text/javascript,export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
                const namespace = await runInThisContext(`import(${JSON.stringify(url)})`, {
                  importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
                });
                const direct = { __proto__: null, [inspect.custom]() { return namespace; } };
                const named = function () {};
                Object.defineProperty(named, "name", { value: namespace });
                const input = { __proto__: null, [inspect.custom]() { return named; } };
                for (const call of calls(basename, direct)) {
                  const error = thrown(call);
                  expect(error).toHaveExactConstructor(TypeError);
                }
                for (const call of calls(basename, input)) {
                  const error = thrown(call);
                  expect(error).toHaveExactConstructor(Error);
                  expect(error).not.toBe(17);
                }
              });
            });

            for (const [dependency, makeInput] of invalidArgumentEffects) {
              describe(`invalid path/suffix diagnostic: ${dependency}`, () => {
                it("propagates supplied effect", { tags: ["node-only"] }, async () => {
                  const basename = await implementation();
                  const sentinel = { dependency };
                  // Both parameter sites reach validateString's same formatter.
                  for (const call of calls(basename, makeInput(sentinel))) {
                    expect(thrown(call)).toBe(sentinel);
                  }
                });
              });
            }

            describe("diagnostic collection iteration propagation", () => {
              it("throws non-Error value", { tags: ["node-only"] }, async () => {
                const basename = await implementation();
                const { inspect } = await import("node:util");
                const sentinel = { marker: "iterator" };
                for (const collection of [new Set([1]), new Map([[1, 2]])]) {
                  Object.defineProperty(collection, Symbol.iterator, { value() { throw sentinel; } });
                  const named = function () {};
                  Object.defineProperty(named, "name", { value: collection });
                  const input = { __proto__: null, [inspect.custom]() { return named; } };
                  for (const call of calls(basename, input)) expect(thrown(call)).toBe(sentinel);
                }
              });
            });

            it("maps platform exports and accepts strings without receiver checks", { tags: ["node-only"] }, async () => {
              const path = await import("node:path");
              const alias = platform === "posix" ? await import("node:path/posix") : await import("node:path/win32");
              const basename = await implementation();
              expect(alias.basename).toBe(basename);
              expect(path[platform][platform].basename).toBe(basename);
              expect(path.basename).toBe(path[process.platform === "win32" ? "win32" : "posix"].basename);
              // Receiver is not an effect dependency; path/suffix are already primitive strings.
              const receiver = new Proxy({}, { get() { throw new Error("receiver must not be read"); } });
              expect(basename.call(receiver, "file.txt", ".txt")).toBe("file");
            });
          });
        }
      });

      describe("dirname", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("extname", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("format", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("isAbsolute", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("join", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("matchesGlob", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("normalize", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("parse", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("relative", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("resolve", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("toNamespacedPath", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("basename", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("dirname", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("extname", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("format", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("isAbsolute", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("join", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("matchesGlob", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("normalize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("parse", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("relative", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("resolve", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toNamespacedPath", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });
});
