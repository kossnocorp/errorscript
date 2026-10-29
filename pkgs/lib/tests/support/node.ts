import { execFileSync, type ExecFileSyncOptionsWithStringEncoding } from "node:child_process";

/** Run real Node in a fresh realm; nonzero exit, signal, or timeout fails the repro. */
export function runNode(
  args: string[],
  options: Omit<ExecFileSyncOptionsWithStringEncoding, "encoding"> = {},
): string {
  return execFileSync(process.execPath, args, {
    timeout: 10_000,
    ...options,
    encoding: "utf8",
  });
}

/** Inject into native Node probes, whose error constructors belong to that realm. */
export const nativeAssertions = `
  const exact = (call, constructor) => assert.throws(call, error => {
    assert.equal(error.constructor, constructor);
    return true;
  });
  const same = (call, value) => assert.throws(call, error => {
    assert.equal(error, value);
    return true;
  });
`;
