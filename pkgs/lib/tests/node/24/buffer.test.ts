// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import { captureThrown } from "../../support/errors";

describe("node:buffer", () => {
  describe("Blob", () => {
    describe("arrayBuffer", () => {
      it("throws TypeError", { tags: ["node-only"] }, async () => {
        const { Blob } = await import("node:buffer");
        for (const receiver of [undefined, null, {}, 1, "blob", Blob.prototype]) {
          // A normal invalid receiver returns a rejected promise, not a throw.
          const result = Reflect.apply(Blob.prototype.arrayBuffer, receiver, []);
          await expect(result).rejects.toHaveExactConstructor(TypeError);
        }
      });

      describe("revoked receiver proxy (synchronous)", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const { Blob } = await import("node:buffer");
          const { proxy, revoke } = Proxy.revocable(new Blob([]), {});
          revoke();
          expect(captureThrown(() => Blob.prototype.arrayBuffer.call(proxy))).toHaveExactConstructor(TypeError);
        });
      });

      for (const failingRead of [1, 2]) {
        describe(`receiver handle lookup ${failingRead} (synchronous propagation)`, () => {
          it("throws non-Error value", { tags: ["node-only"] }, async () => {
            const { Blob } = await import("node:buffer");
            const sentinel = { supplied: true };
            let reads = 0;
            const blob = new Blob([]);
            const handle = Object.getOwnPropertySymbols(blob).find(key => key.description === "kHandle");
            expect(handle).toBeDefined();
            const receiver = new Proxy(blob, {
              get(target, key, receiver) {
                if (key === handle && ++reads === failingRead) throw sentinel;
                return Reflect.get(target, key, receiver);
              },
            });
            expect(captureThrown(() => Blob.prototype.arrayBuffer.call(receiver))).toBe(sentinel);
            expect(reads).toBe(failingRead);
          });
        });
      }

      for (const condition of ["changed", "removed"] as const) {
        describe(`file backing ${condition} (rejection)`, () => {
          it("throws DOMException", { tags: ["node-only"] }, async () => {
            const { openAsBlob } = await import("node:fs");
            const { mkdtemp, writeFile, unlink, rm } = await import("node:fs/promises");
            const { tmpdir } = await import("node:os");
            const { join } = await import("node:path");
            const dir = await mkdtemp(join(tmpdir(), "errs-blob-arraybuffer-"));
            try {
              const path = join(dir, "data");
              await writeFile(path, "original");
              const blob = await openAsBlob(path);
              if (condition === "changed") {
                // Size change is deterministic even with coarse filesystem timestamps.
                await writeFile(path, "different length");
              } else {
                await unlink(path);
              }
              const result = blob.arrayBuffer();
              await expect(result).rejects.toHaveExactConstructor(DOMException);
              // DOMException names identify distinct Web API error categories.
              await expect(result).rejects.toHaveProperty("name", "NotReadableError");
            } finally {
              await rm(dir, { recursive: true, force: true });
            }
          });
        });
      }
    });

    describe("bytes", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("slice", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("stream", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("text", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("textStream", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("File", () => {
    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("WithImplicitCoercion", () => {
    describe("[Symbol.toPrimitive]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("valueOf", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("global", () => {
    describe("Buffer", () => {
      describe("byteLength", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("compare", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("copy", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("equals", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("fill", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("includes", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("indexOf", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("isBuffer", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("isEncoding", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("lastIndexOf", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigInt64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigInt64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigUInt64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigUInt64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigUint64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readBigUint64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readDoubleBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readDoubleLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readFloatBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readFloatLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readInt16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readInt16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readInt32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readInt32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readInt8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readIntBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readIntLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUInt16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUInt16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUInt32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUInt32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUInt8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUIntBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUIntLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUint16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUint16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUint32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUint32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUint8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUintBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("readUintLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("reverse", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("swap16", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("swap32", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("swap64", () => {
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

      describe("write", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigInt64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigInt64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigUInt64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigUInt64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigUint64BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeBigUint64LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeDoubleBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeDoubleLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeFloatBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeFloatLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeInt16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeInt16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeInt32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeInt32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeInt8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeIntBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeIntLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUInt16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUInt16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUInt32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUInt32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUInt8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUIntBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUIntLE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUint16BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUint16LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUint32BE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUint32LE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUint8", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUintBE", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("writeUintLE", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("BufferConstructor", () => {
      describe("byteLength", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("compare", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("isBuffer", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("isEncoding", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("atob", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("btoa", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("isAscii", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("isUtf8", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("resolveObjectURL", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("transcode", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
