// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { captureThrown } from "../../../support/errors";
import strict, { AssertionError } from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createRequire, getSourceMapsSupport, setSourceMapsSupport } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { compileFunction } from "node:vm";
import { describe, expect, it } from "vitest";

// AssertionError changes stackTraceLimit before coercing a supplied message.
// Restore it even when that coercion exits abruptly inside Node.
const thrown = captureThrown;

function mappedFailure(source: string): unknown {
  const support = getSourceMapsSupport();
  const dir = mkdtempSync(join(tmpdir(), "errs-strict-"));
  const file = join(dir, "fixture.cjs");
  const require = createRequire(import.meta.url);
  try {
    setSourceMapsSupport(true);
    const map = Buffer.from(JSON.stringify({
      version: 3, sources: [source], names: [], mappings: "AAAA",
    })).toString("base64");
    writeFileSync(file, `module.exports = strict => strict(false);\n//# sourceMappingURL=data:application/json;base64,${map}\n`);
    // Loading the valid fixture is outside the captured call.
    const call = require(file) as (assert: typeof strict) => void;
    return thrown(() => call(strict));
  } finally {
    delete require.cache[file];
    rmSync(dir, { recursive: true, force: true });
    setSourceMapsSupport(support.enabled, support);
  }
}

describe("node:assert/strict", () => {
  describe("strict", () => {
    it("throws AssertionError", () => {
      const error = thrown(() => strict(false, "explicit failure"));
      expect(error).toHaveExactConstructor(AssertionError);
      const hostile = new Proxy({}, { get() { throw "unused"; } });
      expect(() => Reflect.apply(strict, hostile, [hostile, hostile])).not.toThrow();
    });

    it("throws AssertionError", () => {
      const error = thrown(() => Reflect.apply(strict, null, []));
      expect(error).toHaveExactConstructor(AssertionError);
    });

    it("throws AssertionError", () => {
      const error = thrown(() => strict(0));
      expect(error).toHaveExactConstructor(AssertionError);
    });

    it("throws TypeError", () => {
      const message = Object.create(null) as string;
      expect(thrown(() => strict(false, message))).toHaveExactConstructor(TypeError);
    });

    it("throws AssertionError", () => {
      const support = getSourceMapsSupport();
      const call = compileFunction("strict(0, null);", ["strict"]);
      try {
        // No source map for this generated function: use primitive inspection.
        setSourceMapsSupport(true);
        const error = thrown(() => call(strict));
        expect(error).toHaveExactConstructor(AssertionError);
      } finally {
        setSourceMapsSupport(support.enabled, support);
      }
    });

    it("throws TypeError", () => {
      const { proxy, revoke } = Proxy.revocable({}, {});
      revoke();
      expect(thrown(() => strict(false, proxy as string))).toHaveExactConstructor(TypeError);
    });

    it("throws SyntaxError", () => {
      const support = getSourceMapsSupport();
      // Valid JS, but Node tokenizes only the first source line for its message.
      const call = compileFunction("strict(false /*\nvalid multiline comment */);", ["strict"]);
      try {
        setSourceMapsSupport(false);
        const error = thrown(() => call(strict));
        expect(error).toHaveExactConstructor(SyntaxError);
      } finally {
        setSourceMapsSupport(support.enabled, support);
      }
    });

    it("throws URIError", () => {
      expect(mappedFailure("file:///errs-strict-%zz.ts")).toHaveExactConstructor(URIError);
    });

    it("throws TypeError", () => {
      const error = mappedFailure("file:///errs-strict-%2F.ts");
      expect(error).toHaveExactConstructor(TypeError);
    });

    it("throws Error", () => {
      const supplied = new Error("supplied, not created by strict");
      const error = thrown(() => strict(false, supplied));
      expect(error).toBe(supplied);
    });

    it("throws non-Error value", () => {
      const supplied = { origin: "message conversion" };
      const message = { [Symbol.toPrimitive]() { throw supplied; } };
      expect(thrown(() => strict(false, message as unknown as string))).toBe(supplied);
    });

    it("throws non-Error value", () => {
      const supplied = Symbol("prototype trap");
      const message = new Proxy({}, { getPrototypeOf() { throw supplied; } });
      expect(thrown(() => strict(false, message as string))).toBe(supplied);
    });

    it("throws non-Error value", () => {
      const supplied = { origin: "conversion getter" };
      const message = { get [Symbol.toPrimitive]() { throw supplied; } };
      expect(thrown(() => strict(false, message as unknown as string))).toBe(supplied);
    });

    it("throws non-Error value", () => {
      const supplied = { origin: "prepareStackTrace" };
      const descriptor = Object.getOwnPropertyDescriptor(Error, "prepareStackTrace");
      try {
        Error.prepareStackTrace = () => { throw supplied; };
        expect(thrown(() => strict(false, "explicit"))).toBe(supplied);
      } finally {
        if (descriptor) Object.defineProperty(Error, "prepareStackTrace", descriptor);
        else Reflect.deleteProperty(Error, "prepareStackTrace");
      }
    });
  });
});
