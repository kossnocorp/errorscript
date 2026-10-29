// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown, expectThrown, throwOnGet } from "../../support/errors";
import { describe, expect, it } from "vitest";
import { BlockList, SocketAddress } from "node:net";
import { inspect } from "node:util";
import { runNode } from "../../support/node";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

describe("node:net", () => {
  describe("BlockList", () => {
    describe("addAddress", () => {
      const invoke = (receiver: unknown, address: unknown, family?: unknown) =>
        Reflect.apply(BlockList.prototype.addAddress, receiver, [address, family]);
      const capture = captureThrown;
      const exact = (fn: () => unknown, constructor: Function) => {
        return expectThrown(fn, constructor);
      };

      it("throws TypeError", { tags: ["node-only"] }, () => {
        const list = new BlockList();
        for (const address of [undefined, null, 1, false, 1n, Symbol(), {}, []]) {
          exact(() => invoke(list, address), TypeError);
        }
        for (const family of [null, 4, {}, Symbol()]) {
          exact(() => invoke(list, "127.0.0.1", family), TypeError);
        }
        for (const family of ["", "IPv5", "inet"]) {
          exact(() => invoke(list, "127.0.0.1", family), TypeError);
        }
      });

      it("throws TypeError", { tags: ["node-only"] }, () => {
        const address = new SocketAddress({ address: "127.0.0.1" });
        for (const receiver of [undefined, null, {}, 1, Object.create(BlockList.prototype)]) {
          exact(() => invoke(receiver, address), TypeError);
        }
        const revoked = Proxy.revocable({}, {});
        revoked.revoke();
        exact(() => invoke(new BlockList(), revoked.proxy), TypeError);
        exact(() => invoke(new BlockList(), { constructor: 1 }), TypeError);
      });

      it("throws Error", { tags: ["node-only"] }, () => {
        const list = new BlockList();
        for (const [address, family] of [
          ["not-an-ip", "ipv4"],
          ["256.0.0.1", "ipv4"],
          ["::1", "ipv4"],
          ["not-an-ip", "ipv6"],
          ["127.0.0.1", "ipv6"],
          ["", "ipv6"],
        ]) {
          exact(() => invoke(list, address, family), Error);
        }
        // Both overloads, default family, case folding, and ignored family on
        // the SocketAddress branch. These are controls, not a never-throws proof.
        expect(invoke(list, "127.0.0.1")).toBeUndefined();
        expect(invoke(list, "::1", "IPv6")).toBeUndefined();
        expect(invoke(list, new SocketAddress({ address: "127.0.0.2" }), {})).toBeUndefined();
      });

      it("throws RangeError", { tags: ["node-only"] }, () => {
        // The hooks return, rather than throwing or recursively calling themselves.
        // Node's diagnostic formatter recursively follows their returned objects.
        const first = { constructor: null, [inspect.custom]: () => second };
        const second = { [inspect.custom]: () => first };
        exact(() => invoke(new BlockList(), first), RangeError);
      });

      it("throws Error", { tags: ["node-only"] }, () => {
        const supplied = Object.freeze({ marker: "ownKeys" });
        let calls = 0;
        const inner = new Proxy(
          {},
          {
            ownKeys() {
              // ObjectGetOwnPropertySymbols succeeds; ObjectKeys then throws.
              if (++calls === 2) throw supplied;
              return [];
            },
          },
        );
        const address = {
          constructor: null,
          [inspect.custom]: () => new Proxy(inner, {}),
        };
        const error = exact(
          () => invoke(new BlockList(), address),
          Error,
        );
        expect(calls).toBe(2);
        expect(error).not.toBe(supplied);
      });

      it("throws ReferenceError", { tags: ["node-only"] }, () => {
        // A real ESM namespace is required: a Vite-transformed module is not a
        // faithful fixture for an uninitialized live export binding.
        const directory = mkdtempSync(join(tmpdir(), "errs-lib-add-address-"));
        try {
          const file = join(directory, "tdz.mjs");
          writeFileSync(
            file,
            `
            import * as self from './tdz.mjs';
            import { BlockList } from 'node:net';
            import assert from 'node:assert/strict';
            let caught;
            try { new BlockList().addAddress(self); } catch (error) { caught = error; }
            assert.equal(caught?.constructor, ReferenceError);
            export const constructor = 0;
            console.log('ReferenceError verified');
          `,
          );
          expect(runNode([file])).toBe(
            "ReferenceError verified\n",
          );
        } finally {
          rmSync(directory, { recursive: true, force: true });
        }
      });

      it("throws non-Error value", { tags: ["node-only"] }, () => {
        const supplied = Object.freeze({ marker: "propagated" });
        const fail = () => {
          throw supplied;
        };
        const list = new BlockList();
        const address = new SocketAddress({ address: "127.0.0.1" });
        const listHandle = Object.getOwnPropertySymbols(list).find(key => key.description === "kHandle");
        const addressHandle = Object.getOwnPropertySymbols(address).find(key => key.description === "kHandle");
        expect(listHandle).toBeDefined();
        expect(addressHandle).toBeDefined();
        // Initial symbol-brand read, receiver handle read, and diagnostic reads.
        expect(capture(() => invoke(list, throwOnGet({}, addressHandle!, supplied)))).toBe(supplied);
        expect(capture(() => invoke(throwOnGet(list, listHandle!, supplied), "127.0.0.1"))).toBe(supplied);
        expect(
          capture(() =>
            invoke(list, {
              get constructor() {
                return fail();
              },
            }),
          ),
        ).toBe(supplied);
        expect(
          capture(() =>
            invoke(list, {
              constructor: {
                get name() {
                  return fail();
                },
              },
            }),
          ),
        ).toBe(supplied);
        expect(
          capture(() => invoke(list, "127.0.0.1", { constructor: null, [inspect.custom]: fail })),
        ).toBe(supplied);
        let reads = 0;
        const proxy = new Proxy(address, {
          get(target, key, receiver) {
            // Same symbol is read first for branding, then for the native handle argument.
            if (key === addressHandle && ++reads === 2) throw supplied;
            return Reflect.get(target, key, receiver);
          },
        });
        expect(capture(() => invoke(list, proxy))).toBe(supplied);
        expect(reads).toBe(2);
      });
    });

    describe("addRange", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("addSubnet", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("check", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("fromJSON", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static isBlockList", () => {
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

  describe("BoundSocket", () => {
    describe("[Symbol.dispose]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("address", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("close", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("fd", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("ListenOptions", () => {
    describe("handle", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });
  });

  describe("LookupFunction", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("OnReadOpts", () => {
    describe("buffer", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("callback", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("Server", () => {
    describe("[Symbol.asyncDispose]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("address", () => {
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

    describe("getConnections", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("listen", () => {
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

    describe("ref", () => {
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

  describe("Socket", () => {
    describe("addListener", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("address", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("connect", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("destroySoon", () => {
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

    describe("getTypeOfService", () => {
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

    describe("pause", () => {
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

    describe("resetAndDestroy", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("resume", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setEncoding", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setKeepAlive", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setNoDelay", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTimeout", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("setTypeOfService", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("unref", () => {
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

  describe("SocketAddress", () => {
    describe("new", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("static parse", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("SocketConstructorOpts", () => {
    describe("handle", () => {
      describe("new", () => {
        it("TODO", () => {
          // TODO
        });
      });
    });
  });

  describe("TcpSocketConnectOpts", () => {
    describe("lookup", () => {
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

  describe("createConnection", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("createServer", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getDefaultAutoSelectFamily", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("getDefaultAutoSelectFamilyAttemptTimeout", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("isIP", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("isIPv4", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("isIPv6", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setDefaultAutoSelectFamily", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("setDefaultAutoSelectFamilyAttemptTimeout", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
