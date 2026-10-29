// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown } from "../../support/errors";
import { describe, expect, it } from "vitest";

describe("node:perf_hooks", () => {
  describe("ELDHistogram", () => {
    describe("[Symbol.dispose]", () => {
      const thrownBy = captureThrown;

      describe("invalid receiver or non-callable disable", () => {
        it("throws TypeError", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          const dispose = histogram[Symbol.dispose];
          try {
            for (const receiver of [
              undefined, null, 1, true, "", 1n, Symbol(), {}, { disable: 0 },
            ]) {
              const error = thrownBy(() => Reflect.apply(dispose, receiver, []));
              expect(error).toHaveExactConstructor(TypeError);
            }
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      describe("native module namespace with uninitialized disable export", () => {
        it("throws ReferenceError", async () => {
          const { spawnSync } = await import("node:child_process");
          const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
          const { tmpdir } = await import("node:os");
          const { join } = await import("node:path");
          const directory = await mkdtemp(join(tmpdir(), "errs-eld-dispose-"));
          try {
            const filename = join(directory, "fixture.mjs");
            await writeFile(filename, `
import assert from 'node:assert/strict';
import { monitorEventLoopDelay } from 'node:perf_hooks';
import * as receiver from './fixture.mjs';

const histogram = monitorEventLoopDelay();
const dispose = histogram[Symbol.dispose];
let caught;
try {
  Reflect.apply(dispose, receiver, []);
} catch (error) {
  caught = error;
} finally {
  histogram[Symbol.dispose]();
}
assert.equal(caught?.constructor, ReferenceError);
export let disable = () => {};
assert.equal(typeof receiver.disable, 'function');
console.log(JSON.stringify({ node: process.version, v8: process.versions.v8, name: caught.name }));
`);
            const child = spawnSync(process.execPath, [filename], {
              encoding: "utf8",
              timeout: 10_000,
              env: { ...process.env, NODE_OPTIONS: "" },
            });
            expect(child.error).toBeUndefined();
            expect(child.stderr).toBe("");
            expect(child.signal).toBeNull();
            expect(child.status).toBe(0);
            expect(JSON.parse(child.stdout)).toEqual({
              node: process.version,
              v8: process.versions.v8,
              name: "ReferenceError",
            });
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });
      });

      describe("delegated disable receiver check", () => {
        it("throws TypeError", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          try {
            const error = thrownBy(() =>
              Reflect.apply(histogram[Symbol.dispose], { disable: histogram.disable }, []),
            );
            expect(error).toHaveExactConstructor(TypeError);
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      describe("revoked receiver proxy", () => {
        it("throws TypeError", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          const { proxy, revoke } = Proxy.revocable(histogram, {});
          revoke();
          try {
            const error = thrownBy(() => Reflect.apply(histogram[Symbol.dispose], proxy, []));
            expect(error).toHaveExactConstructor(TypeError);
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      describe("enabled state write rejected", () => {
        it("throws TypeError", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          for (const samplePerIteration of [false, true]) {
            const histogram = monitorEventLoopDelay({ samplePerIteration });
            try {
              const enabled = Object.getOwnPropertySymbols(histogram).find(key => key.description === "kEnabled");
              expect(enabled).toBeDefined();
              histogram.enable();
              const receiver = new Proxy(histogram, {
                set(target, key, value, receiver) {
                  return key === enabled ? false : Reflect.set(target, key, value, receiver);
                },
              });
              const error = thrownBy(() =>
                Reflect.apply(histogram[Symbol.dispose], receiver, []),
              );
              expect(error).toHaveExactConstructor(TypeError);
            } finally {
              histogram[Symbol.dispose]();
            }
          }
        });
      });

      describe("disable getter propagation", () => {
        it("throws non-Error value", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          const sentinel = { source: "disable getter" };
          const receiver = {
            get disable() {
              throw sentinel;
            },
          };
          try {
            expect(thrownBy(() => Reflect.apply(histogram[Symbol.dispose], receiver, []))).toBe(
              sentinel,
            );
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      describe("supplied disable callback propagation", () => {
        it("propagates supplied effect", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          const sentinel = new Error("supplied disable");
          const receiver = {
            disable() {
              throw sentinel;
            },
          };
          try {
            const error = thrownBy(() => Reflect.apply(histogram[Symbol.dispose], receiver, []));
            expect(error).toBe(sentinel);
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      describe("delegated state get and set propagation", () => {
        it("throws non-Error value", async () => {
          const { monitorEventLoopDelay } = await import("node:perf_hooks");
          const histogram = monitorEventLoopDelay();
          const sentinel = { source: "receiver proxy" };
          const enabled = Object.getOwnPropertySymbols(histogram).find(key => key.description === "kEnabled");
          expect(enabled).toBeDefined();
          try {
            const readReceiver = new Proxy(histogram, {
              get(target, key, receiver) {
                if (key === enabled) throw sentinel;
                return Reflect.get(target, key, receiver);
              },
            });
            expect(thrownBy(() => Reflect.apply(histogram[Symbol.dispose], readReceiver, []))).toBe(
              sentinel,
            );
            histogram.enable();
            const writeReceiver = new Proxy(histogram, {
              set(target, key, value, receiver) {
                if (key === enabled) throw sentinel;
                return Reflect.set(target, key, value, receiver);
              },
            });
            expect(thrownBy(() => Reflect.apply(histogram[Symbol.dispose], writeReceiver, []))).toBe(
              sentinel,
            );
          } finally {
            histogram[Symbol.dispose]();
          }
        });
      });

      it("disables both native sampling modes idempotently", async () => {
        const { monitorEventLoopDelay } = await import("node:perf_hooks");
        for (const samplePerIteration of [false, true]) {
          const histogram = monitorEventLoopDelay({ samplePerIteration });
          try {
            expect(histogram[Symbol.dispose]()).toBeUndefined();
            expect(histogram.enable()).toBe(true);
            expect(histogram[Symbol.dispose]()).toBeUndefined();
            expect(histogram.disable()).toBe(false);
            expect(histogram[Symbol.dispose]()).toBeUndefined();
            expect(histogram.enable()).toBe(true);
            expect(histogram[Symbol.dispose]()).toBeUndefined();
          } finally {
            histogram[Symbol.dispose]();
          }
        }
      });

    });

    describe("disable", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("enable", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Histogram", () => {
    describe("percentile", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("percentileBigInt", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("reset", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Performance", () => {
    describe("clearMarks", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("clearMeasures", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("clearResourceTimings", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("eventLoopUtilization", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntries", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByType", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("mark", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("markResourceTiming", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("measure", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("now", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setResourceTimingBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("timerify", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("PerformanceEntry", () => {
    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("PerformanceObserver", () => {
    describe("disconnect", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("observe", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("takeRecords", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("PerformanceObserverCallback", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("PerformanceObserverEntryList", () => {
    describe("getEntries", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByType", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("PerformanceResourceTiming", () => {
    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("RecordableHistogram", () => {
    describe("add", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("record", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("recordDelta", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("createHistogram", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("eventLoopUtilization", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("monitorEventLoopDelay", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("performance", () => {
    describe("clearMarks", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("clearMeasures", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("clearResourceTimings", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("eventLoopUtilization", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntries", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByName", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getEntriesByType", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("mark", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("markResourceTiming", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("measure", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("now", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setResourceTimingBufferSize", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("timerify", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("timerify", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
