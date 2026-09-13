export function example() {
  try {
    throw new Error("Example failure");
  } catch (error) {
    // Hover error: the plugin supplies catch (error: errtype) to TypeScript.
    return error.message;
  }
}

// Ordinary TypeScript diagnostics still work; uncomment to try:
// const wrong: number = "oops";
