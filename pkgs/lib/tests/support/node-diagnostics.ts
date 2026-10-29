import { inspect } from "node:util";
import { throwOnGet } from "./errors";

// Node 24.19.0 ERR_INVALID_ARG_TYPE / determineSpecificType only.
// Each caller must prove it reaches this formatter; these are not generic input effects.
// Proof: docs/evidence/shared/node24-invalid-argument.md.
export const invalidArgumentEffects: ReadonlyArray<readonly [string, (sentinel: unknown) => object]> = [
  ["input.constructor get", (value) => throwOnGet({}, "constructor", value)],
  ["input.constructor has name", (value) => ({
    constructor: new Proxy({ name: "Constructor" }, {
      has(target, key) {
        if (key === "name") throw value;
        return Reflect.has(target, key);
      },
    }),
  })],
  ["input.constructor.name get", (value) => ({ constructor: throwOnGet({ name: "Constructor" }, "name", value) })],
  ["input.constructor.name[Symbol.toPrimitive] get", (value) => ({
    constructor: { name: throwOnGet({}, Symbol.toPrimitive, value) },
  })],
  ["input.constructor.name[Symbol.toPrimitive] call", (value) => ({
    constructor: { name: { [Symbol.toPrimitive]() { throw value; } } },
  })],
  ["input.constructor.name.toString get", (value) => ({
    constructor: { name: throwOnGet({}, "toString", value) },
  })],
  ["input.constructor.name.toString call", (value) => ({
    constructor: { name: { toString() { throw value; } } },
  })],
  ["input.constructor.name.valueOf call after nonprimitive toString", (value) => ({
    constructor: { name: { toString() { return {}; }, valueOf() { throw value; } } },
  })],
  ["input.constructor.name.valueOf get after nonprimitive toString", (value) => ({
    constructor: { name: throwOnGet({ toString() { return {}; } }, "valueOf", value) },
  })],
  ["function input.name get", (value) => throwOnGet(function named() {}, "name", value)],
  ["function input.name coercion", (value) => Object.defineProperty(function named() {}, "name", {
    value: { [Symbol.toPrimitive]() { throw value; } },
  })],
  // inspect unwraps proxies; use an actual accessor on the inspected target.
  ["input[inspect.custom] get after constructor fallback", (value) =>
    Object.create(null, { [inspect.custom]: { get() { throw value; } } })],
  ["input[inspect.custom] call after constructor fallback", (value) => ({
    __proto__: null, [inspect.custom]() { throw value; },
  })],
];
