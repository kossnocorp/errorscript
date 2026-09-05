const { xxh32 } = require("@js-fns/xxhash");
const { hello } = require("./mod.js");

function throws() {
  throw new Error("This function always throws an error.");
}

function neverThrows() {
  return "This function never throws an error.";
}

function helloFriend() {
  return hello("friend");
}

export function hasher(input) {
  return xxh32(input);
}

module.exports = { throws, neverThrows, helloFriend, hasher };
