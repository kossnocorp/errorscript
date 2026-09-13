import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const compiler = join(dirname(require.resolve("typescript/package.json")), "bin/tsc");
const mapper = fileURLToPath(new URL("..", import.meta.url));

/**
 * Exercise real compiler package resolution, JSON-RPC, and span mapping.
 * @param {string} content
 * @param {{ extension?: string, mapped?: boolean }} options
 */
function compile(content, { extension = ".ets", mapped = true } = {}) {
  const directory = mkdtempSync(join(tmpdir(), "errorscript-ts-"));
  try {
    mkdirSync(join(directory, "node_modules/@errorscript"), { recursive: true });
    symlinkSync(mapper, join(directory, "node_modules/@errorscript/ts"), "junction");
    writeFileSync(join(directory, `index${extension}`), content);
    writeFileSync(
      join(directory, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: {
          strict: true,
          noEmit: true,
          target: "esnext",
          module: "nodenext",
          moduleDetection: "force",
          types: [],
        },
        contentMappers: mapped ? [{ package: "@errorscript/ts", extensions: [extension] }] : [],
        files: [`index${extension}`],
      }),
    );
    const result = spawnSync(
      process.execPath,
      [compiler, "--project", directory, "--runExternalCode", "--pretty", "false"],
      {
        encoding: "utf8",
        timeout: 15_000,
      },
    );
    assert.ifError(result.error);
    return { status: result.status, output: result.stdout + result.stderr };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("catch bindings become any through the actual mapper process", () => {
  const source = `// Unicode exercises UTF-8 message framing and UTF-16 spans: 🐛\ntry {} catch (error) { error.message; }\ntry {} catch ($error) { $error.arbitraryMethod(); }\ntry {} catch {}\ntry {} catch (error: unknown) { void error; }\n`;
  const plain = compile(source, { extension: ".ts", mapped: false });
  assert.notEqual(plain.status, 0);
  assert.match(plain.output, /TS18046/);
  const mapped = compile(source);
  assert.equal(mapped.status, 0, mapped.output);
});

test("unrelated diagnostics retain original positions after insertions", () => {
  const source = `/* 🐛 */ try {} catch (error) { error.message; } try {} catch (other) { other.message; } const wrong: number = "oops";`;
  const result = compile(source);
  assert.equal(result.status, 2, result.output);
  assert.ok(
    result.output.includes(`index.ets(1,${source.indexOf("wrong") + 1}): error TS2322`),
    result.output,
  );
  assert.doesNotMatch(result.output, /TS18046/);
});

test("the pinned compiler currently rejects mapping built-in .ts files", () => {
  const result = compile("try {} catch (error) { error.message; }", { extension: ".ts" });
  assert.equal(result.status, 2, result.output);
  assert.match(result.output, /TS100021/);
  assert.match(result.output, /built-in extension/);
});
