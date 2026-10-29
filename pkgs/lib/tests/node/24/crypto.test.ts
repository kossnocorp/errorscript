// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, throwOnGet } from "../../support/errors";
import { invalidArgumentEffects } from "../../support/node-diagnostics";
import { describe, expect, it } from "vitest";

describe("node:crypto", () => {
  describe("Certificate", () => {
    describe("exportChallenge", () => {
      // The runtime accepts encoding and buffer sources beyond the declaration.
      const invoke = async () => {
        const { Certificate } = await import("node:crypto");
        return Certificate.prototype.exportChallenge as unknown as (
          spkac?: unknown,
          encoding?: unknown,
        ) => unknown;
      };
      const thrown = captureThrown;

      describe("argument validation", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          // Representatives of primitive and object diagnostics; effectful shapes below.
          for (const value of [undefined, {}]) {
            const error = thrown(() => exportChallenge(value));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("unknown string encoding", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          // Empty strings still validate encoding, before the native empty-input return.
          for (const value of [""]) {
            const error = thrown(() => exportChallenge(value, "not-an-encoding"));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("diagnostic formatting", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          const { proxy, revoke } = Proxy.revocable({}, {});
          revoke();
          for (const value of [proxy, { constructor: 1 }, { constructor: { name: Symbol() } }]) {
            const error = thrown(() => exportChallenge(value));
            expect(error).toHaveExactConstructor(TypeError);
          }
        });
      });

      describe("native signed-int size limit", () => {
        it("throws RangeError", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          // Real zero-filled virtual allocation, rejected before native parsing/copying.
          const input = new ArrayBuffer(2 ** 31);
          try {
            const error = thrown(() => exportChallenge(input));
            expect(error).toHaveExactConstructor(RangeError);
          } finally {
            input.transfer(0);
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
          const directory = await mkdtemp(join(tmpdir(), "export-challenge-"));
          try {
            const file = join(directory, "fixture.mjs");
            await writeFile(file, `
              import * as namespace from './fixture.mjs';
              import { Certificate } from 'node:crypto';
              const stackLimit = Object.getOwnPropertyDescriptor(Error, 'stackTraceLimit');
              let captured;
              try { Certificate.prototype.exportChallenge(namespace); }
              catch (error) { captured = error; }
              finally { Object.defineProperty(Error, 'stackTraceLimit', stackLimit); }
              export { captured };
              export let constructor;
            `);
            const { captured } = await runInThisContext(
              `import(${JSON.stringify(pathToFileURL(file).href)})`,
              { importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER },
            );
            expect(captured).toHaveExactConstructor(ReferenceError);
          } finally {
            await rm(directory, { recursive: true, force: true });
          }
        });
      });

      describe("custom inspection return cycle", () => {
        it("throws RangeError", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          const { inspect } = await import("node:util");
          // Both hooks return normally; Node's diagnostic traversal overflows.
          const first = { __proto__: null, [inspect.custom](): object { return second; } };
          const second = { [inspect.custom](): object { return first; } };
          const error = thrown(() => exportChallenge(first));
          expect(error).toHaveExactConstructor(RangeError);
        });
      });

      describe("namespace formatting through a function name", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const exportChallenge = await invoke();
          const { inspect } = await import("node:util");
          const { runInThisContext, constants } = await import("node:vm");
          const url = 'data:text/javascript,export const value = { [Symbol.for("nodejs.util.inspect.custom")]() { throw 17; } };';
          const namespace = await runInThisContext(`import(${JSON.stringify(url)})`, {
            importModuleDynamically: constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
          });

          // Direct namespace return stops at depth -1: its exported hook is not run.
          const direct = { __proto__: null, [inspect.custom]() { return namespace; } };
          const directError = thrown(() => exportChallenge(direct));
          expect(directError).toHaveExactConstructor(TypeError);

          const named = function () {};
          Object.defineProperty(named, "name", { value: namespace });
          const input = { __proto__: null, [inspect.custom]() { return named; } };
          const error = thrown(() => exportChallenge(input));
          expect(error).toHaveExactConstructor(Error);
          expect(error).not.toBe(17);
        });
      });

      for (const [dependency, makeInput] of invalidArgumentEffects) {
        describe(`invalid spkac diagnostic: ${dependency}`, () => {
          it("propagates supplied effect", { tags: ["node-only"] }, async () => {
            const exportChallenge = await invoke();
            const sentinel = { dependency };
            expect(thrown(() => exportChallenge(makeInput(sentinel)))).toBe(sentinel);
          });
        });
      }

      describe("declared buffer-view shape without native view brand", () => {
        it("propagates supplied effect", { tags: ["node-only"] }, async () => {
          const { Certificate } = await import("node:crypto");
          const sentinel = { effect: "proxied view constructor read" };
          const input = throwOnGet(new Uint8Array(), "constructor", sentinel);
          // No cast: a typed proxy can still fail ArrayBuffer.isView at runtime.
          expect(thrown(() => Certificate.prototype.exportChallenge(input))).toBe(sentinel);
        });
      });

      it("accepts unbranded receivers and ignores non-string encodings", { tags: ["node-only"] }, async () => {
        const exportChallenge = await invoke();
        const encoding = new Proxy({}, { get() { throw new Error("must not coerce encoding"); } });
        expect(exportChallenge.call(null, "", encoding)).toBe("");
        expect(exportChallenge("invalid SPKAC", "buffer")).toBe("");
        expect(exportChallenge(new Uint8Array(), "not-an-encoding")).toBe("");
        const input = new ArrayBuffer(1);
        input.transfer(0);
        expect(exportChallenge(input)).toBe("");
      });
    });

    describe("exportPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static exportChallenge", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static exportPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static verifySpkac", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("verifySpkac", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("CipherCCM", () => {
    describe("getAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("CipherChaCha20Poly1305", () => {
    describe("getAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("CipherGCM", () => {
    describe("getAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("CipherKey", () => {
    describe("equals", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("export", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static from", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toCryptoKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("CipherOCB", () => {
    describe("getAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Cipheriv", () => {
    describe("final", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAutoPadding", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("update", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DecipherCCM", () => {
    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DecipherChaCha20Poly1305", () => {
    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DecipherGCM", () => {
    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DecipherOCB", () => {
    describe("setAAD", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAuthTag", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Decipheriv", () => {
    describe("final", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setAutoPadding", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("update", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DiffieHellman", () => {
    describe("computeSecret", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("generateKeys", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getGenerator", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPrime", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPrivateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setPrivateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DiffieHellmanGroup", () => {
    it("TODO", () => {
      // TODO
    });
    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("DiffieHellmanGroupConstructor", () => {
    it("TODO", () => {
      // TODO
    });
    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ECDH", () => {
    describe("computeSecret", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("generateKeys", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPrivateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setPrivateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static convertKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Hash", () => {
    describe("copy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("digest", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("update", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Hmac", () => {
    describe("digest", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("update", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("KeyLike", () => {
    describe("equals", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("export", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static from", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toCryptoKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("KeyObject", () => {
    describe("equals", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("export", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static from", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toCryptoKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Sign", () => {
    describe("sign", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("update", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Verify", () => {
    describe("update", () => {
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

  describe("X509Certificate", () => {
    describe("checkEmail", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("checkHost", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("checkIP", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("checkIssued", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("checkPrivateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("issuerCertificate", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toLegacyObject", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("toString", () => {
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

  describe("argon2", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("argon2Sync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("checkPrime", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("checkPrimeSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createCipheriv", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createDecipheriv", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createDiffieHellman", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createDiffieHellmanGroup", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createECDH", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createHash", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createHmac", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createPrivateKey", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createPublicKey", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createSecretKey", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createSign", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createVerify", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("decapsulate", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("diffieHellman", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("encapsulate", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("generateKey", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("generateKeyPair", () => {
    it("TODO", () => {
      // TODO
    });
    describe("__promisify__", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("generateKeyPairSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("generateKeySync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("generatePrime", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("generatePrimeSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getCipherInfo", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getCiphers", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getCurves", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getDiffieHellman", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getFips", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getHashes", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getRandomValues", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("hash", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("hkdf", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("hkdfSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("pbkdf2", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("pbkdf2Sync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("privateDecrypt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("privateEncrypt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("pseudoRandomBytes", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("publicDecrypt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("publicEncrypt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomBytes", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomFill", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomFillSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomInt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomUUID", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("randomUUIDv7", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("scrypt", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("scryptSync", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("secureHeapUsed", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setEngine", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setFips", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("sign", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("subtle", () => {
    describe("decapsulateBits", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("decapsulateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("decrypt", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("deriveBits", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("deriveKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("digest", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("encapsulateBits", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("encapsulateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("encrypt", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("exportKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("generateKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("getPublicKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("importKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("sign", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unwrapKey", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("verify", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("wrapKey", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("timingSafeEqual", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("verify", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("webcrypto", () => {
    describe("Crypto", () => {
      describe("getRandomValues", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("randomUUID", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("SubtleCrypto", () => {
      describe("decapsulateBits", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("decapsulateKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("decrypt", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("deriveBits", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("deriveKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("digest", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("encapsulateBits", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("encapsulateKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("encrypt", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("exportKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("generateKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("getPublicKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("importKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("sign", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("unwrapKey", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("verify", () => {
        it("TODO", () => {
          // TODO
        });
      });

      describe("wrapKey", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });

    describe("getRandomValues", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("randomUUID", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });
});
