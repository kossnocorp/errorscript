import { expect } from "vitest";

declare module "vitest" {
  interface Assertion<R extends void | Promise<void> = void, T = unknown> {
    toHaveExactConstructor(constructor: Function): R;
  }
}

expect.extend({
  toHaveExactConstructor(received: unknown, constructor: Function) {
    const actual = (received as { constructor?: unknown } | null | undefined)?.constructor;
    return {
      pass: actual === constructor,
      message: () => `expected value ${this.isNot ? "not " : ""}to have exact constructor ${constructor.name}`,
      actual,
      expected: constructor,
    };
  },
});

/** Restore even an absent property or an accessor, including when the action throws. */
export function withPropertyRestored<T>(target: object, key: PropertyKey, action: () => T): T {
  const descriptor = Object.getOwnPropertyDescriptor(target, key);
  try {
    return action();
  } finally {
    if (descriptor) Object.defineProperty(target, key, descriptor);
    else Reflect.deleteProperty(target, key);
  }
}

/** Synchronous only; preserves arbitrary thrown values, including undefined. */
export function captureThrown(action: () => unknown): unknown {
  // Node diagnostic formatting can throw before restoring its temporary zero limit.
  return withPropertyRestored(Error, "stackTraceLimit", () => {
    try {
      action();
    } catch (error) {
      return error;
    }
    // Keep the assertion outside the catch so normal returns cannot pass as throws.
    expect.unreachable("Expected a synchronous throw");
  });
}

/** Exact class, rather than instanceof's acceptance of subclasses. */
export function expectThrown(
  action: () => unknown,
  constructor: Function,
): unknown {
  const error = captureThrown(action);
  expect(error).toHaveExactConstructor(constructor);
  return error;
}

/** Fail only the named read so a repro identifies its dependency, not the first trap. */
export function throwOnGet<T extends object>(target: T, key: PropertyKey, sentinel: unknown): T {
  return new Proxy(target, {
    get(object, property, receiver) {
      if (property === key) throw sentinel;
      return Reflect.get(object, property, receiver);
    },
  });
}
