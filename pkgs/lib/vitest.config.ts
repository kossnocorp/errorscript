import { fileURLToPath } from "node:url";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

const nodeVersion = process.env.ERRS_LIB_NODE_VERSION ?? "24";
if (!/^\d+$/.test(nodeVersion)) {
  throw new Error("ERRS_LIB_NODE_VERSION must be a Node major number");
}

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    passWithNoTests: false,
    allowOnly: false,
    isolate: true,
    maxWorkers: 4,
    retry: 0,
    // Pending inventory entries must not inflate the passing repro count.
    // A CLI -t overrides this: focus on "throws ...", not the TODO itself.
    testNamePattern: /^(?!.*(?:^| > )TODO$)/,
    projects: [
      {
        test: {
          name: `node-${nodeVersion}`,
          environment: "node",
          pool: "forks",
          include: [`tests/node/${nodeVersion}/**/*.test.ts`, "tests/ts/**/*.test.ts"],
          exclude: ["tests/ts/webworker*.test.ts", "tests/ts/scripthost.test.ts"],
          setupFiles: ["tests/support/node-version.ts"],
          tags: [
            { name: "node-only", description: "Requires the Node runtime" },
            { name: "browser-only", description: "Requires a real browser", skip: true },
          ],
        },
      },
      {
        test: {
          name: "chromium",
          include: ["tests/ts/**/*.test.ts"],
          // ScriptHost needs its own real host before its TODOs can be resolved.
          exclude: ["tests/ts/scripthost.test.ts"],
          tags: [
            { name: "node-only", description: "Requires the Node runtime", skip: true },
            { name: "browser-only", description: "Requires a real browser" },
          ],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [{ browser: "chromium", name: "chromium" }],
          },
        },
      },
    ],
  },
});
