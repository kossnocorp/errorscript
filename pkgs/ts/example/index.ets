export function example() {
  try {
    throw new Error("Example failure");
  } catch (error) {
    // With strict checking this is normally TS18046: error is unknown.
    return error.message;
  }
}
