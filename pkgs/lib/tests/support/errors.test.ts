import { describe, expect, it } from "vitest";
import { captureThrown, expectThrown, throwOnGet, withPropertyRestored } from "./errors";

describe("repro assertions", () => {
  it("targets a property read while forwarding unrelated access and receiver", () => {
    const key = Symbol("target");
    const target = { other: 42, get receiver() { return this; } };
    const sentinel = {};
    const proxy = throwOnGet(target, key, sentinel);
    expect(proxy.other).toBe(42);
    expect(proxy.receiver).toBe(proxy);
    expect(captureThrown(() => Reflect.get(proxy, key))).toBe(sentinel);
  });
  it("rejects a normally returning callable", () => {
    expect(() => captureThrown(() => undefined)).toThrow("Expected a synchronous throw");
  });

  it("preserves arbitrary thrown values by identity", () => {
    for (const value of [undefined, null, 0, "failure", Symbol("failure"), {}]) {
      expect(captureThrown(() => { throw value; })).toBe(value);
    }
  });

  it("requires the exact constructor, rejecting subclasses and nullish values", async () => {
    const error = Object.assign(new TypeError("failure"), { code: "ERR_EXAMPLE" });
    const call = () => { throw error; };
    expect(expectThrown(call, TypeError)).toBe(error);
    expect(() => expectThrown(call, Error)).toThrow();
    expect(() => expect(undefined).toHaveExactConstructor(TypeError)).toThrow();
    expect(() => expect(null).toHaveExactConstructor(TypeError)).toThrow();
    expect(error).not.toHaveExactConstructor(Error);
    await expect(Promise.reject(error)).rejects.toHaveExactConstructor(TypeError);
  });

  it("restores complete descriptors and removes initially absent properties", () => {
    const target = {};
    const descriptor = { get: () => 42, configurable: true, enumerable: false };
    Object.defineProperty(target, "value", descriptor);
    const sentinel = {};
    expect(captureThrown(() => withPropertyRestored(target, "value", () => {
      Object.defineProperty(target, "value", { value: 0, configurable: true });
      throw sentinel;
    }))).toBe(sentinel);
    expect(Object.getOwnPropertyDescriptor(target, "value")).toEqual(descriptor);
    expect(withPropertyRestored(target, "missing", () => {
      Object.defineProperty(target, "missing", { value: 1, configurable: true });
      return 42;
    })).toBe(42);
    expect(Object.hasOwn(target, "missing")).toBe(false);
  });

  it("restores stack limits after diagnostic failure", () => {
    const before = Object.getOwnPropertyDescriptor(Error, "stackTraceLimit");
    const sentinel = {};
    expect(captureThrown(() => {
      Object.defineProperty(Error, "stackTraceLimit", { value: 0, configurable: true });
      throw sentinel;
    })).toBe(sentinel);
    expect(Object.getOwnPropertyDescriptor(Error, "stackTraceLimit")).toEqual(before);
  });
});
