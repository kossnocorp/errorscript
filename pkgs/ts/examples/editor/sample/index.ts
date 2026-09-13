export function example() {
  try {
    throw new Error("Example failure");
  } catch (error) {
    // Hover error: TypeScript 7 sees the virtual catch (error: errtype).
    return error.message;
  }
}

// This intentional error should be underlined at the original source location.
export const wrong: number = "oops";

// Try annotating a catch binding with unknown: its diagnostics should reappear.
// Use "ErrorScript TS7: Show Transformed Source" to inspect the inserted alias.
