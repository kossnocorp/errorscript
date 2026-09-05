import { xxh32 } from "@js-fns/xxhash";
import { hello } from "./mod.js";

export function throws() {
  throw new Error("This function always throws an error.");
}

export function neverThrows() {
  return "This function never throws an error.";
}

export function helloFriend() {
  return hello("friend");
}

export function hasher(input) {
  return xxh32(input);
}
