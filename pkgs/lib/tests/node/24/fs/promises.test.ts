// Synced by errs-lib. Describe paths are managed; test bodies are yours.
import { describe, expect, it } from "vitest";
import "../../../support/errors";

describe("node:fs/promises", () => {
  describe("DisposableTempDir", () => {
    describe("[Symbol.asyncDispose]", () => {
      describe("captured ASCII-encoded path contains NUL", () => {
        it("throws TypeError", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          try {
            const dir = await fs.mkdtempDisposable(join(root, "\u0100-"), "ascii");
            expect(dir.path).toContain("\0");
            await expect(dir[Symbol.asyncDispose]()).rejects.toHaveExactConstructor(TypeError);
          } finally {
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });

      describe("ancestor replaced by a file", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          const parent = join(root, "parent");
          try {
            await fs.mkdir(parent);
            const dir = await fs.mkdtempDisposable(join(parent, "child-"));
            await fs.rename(parent, join(root, "moved"));
            await fs.writeFile(parent, "not a directory");
            // The method uses its captured path, not this, path, or remove.
            const hostile = new Proxy(
              {},
              {
                get() {
                  throw "must not be read";
                },
              },
            );
            const result = Reflect.apply(dir[Symbol.asyncDispose], hostile, [hostile]);
            await expect(result).rejects.toHaveExactConstructor(Error);
          } finally {
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });

      describe("parent is not writable", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          try {
            const dir = await fs.mkdtempDisposable(join(root, "child-"));
            await fs.chmod(root, 0o500);
            await expect(dir[Symbol.asyncDispose]()).rejects.toHaveExactConstructor(Error);
          } finally {
            await fs.chmod(root, 0o700);
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });

      describe("recursive child is not readable", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          const child = join(root, "child");
          let locked: string | undefined;
          try {
            const dir = await fs.mkdtempDisposable(child);
            locked = join(dir.path, "locked");
            await fs.mkdir(locked);
            await fs.writeFile(join(locked, "file"), "content");
            await fs.chmod(locked, 0o000);
            await expect(dir[Symbol.asyncDispose]()).rejects.toHaveExactConstructor(Error);
          } finally {
            if (locked) await fs.chmod(locked, 0o700);
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });

      describe("permission-model readdir denial", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const { execFile } = await import("node:child_process");
          const { promisify } = await import("node:util");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          try {
            const { stdout } = await promisify(execFile)(
              process.execPath,
              [
                "--permission",
                `--allow-fs-write=${root}/*`,
                "--input-type=module",
                "-e",
                `import assert from 'node:assert/strict';
               import { mkdtempDisposable, writeFile } from 'node:fs/promises';
               const dir = await mkdtempDisposable(${JSON.stringify(join(root, "child-"))});
               await writeFile(dir.path + '/file', 'content');
               const result = dir[Symbol.asyncDispose]();
               assert.ok(result instanceof Promise);
               await assert.rejects(result, error => {
                 assert.equal(error.constructor, Error);
                 assert.equal(error.permission, 'FileSystemRead');
                 assert.equal(error.resource, dir.path);
                 return true;
               });
               console.log('rejection verified');`,
              ],
              { timeout: 5000 },
            );
            expect(stdout.trim()).toBe("rejection verified");
          } finally {
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });

      describe("permission-model rmdir denial escapes its callback", () => {
        it("throws Error", { tags: ["node-only"] }, async () => {
          const fs = await import("node:fs/promises");
          const { join } = await import("node:path");
          const { tmpdir } = await import("node:os");
          const { execFile } = await import("node:child_process");
          const { promisify } = await import("node:util");
          const root = await fs.mkdtemp(join(tmpdir(), "errs-dispose-"));
          const prefix = join(root, "child-");
          try {
            const { stdout } = await promisify(execFile)(
              process.execPath,
              [
                "--permission",
                "--allow-fs-read=*",
                `--allow-fs-write=${prefix}XXXXXX`,
                "--input-type=module",
                "-e",
                `import assert from 'node:assert/strict';
               import { mkdtempDisposable } from 'node:fs/promises';
               const dir = await mkdtempDisposable(${JSON.stringify(prefix)});
               let settled = false;
               process.once('uncaughtException', (error, origin) => {
                 assert.equal(origin, 'uncaughtException');
                 assert.equal(error.constructor, Error);
                 assert.equal(error.permission, 'FileSystemWrite');
                 assert.equal(error.resource, dir.path);
                 assert.equal(settled, false);
                 console.log('callback throw verified');
               });
               const result = dir[Symbol.asyncDispose]();
               assert.ok(result instanceof Promise);
               result.then(() => { settled = true; }, () => { settled = true; });`,
              ],
              { timeout: 5000 },
            );
            expect(stdout.trim()).toBe("callback throw verified");
          } finally {
            await fs.rm(root, { recursive: true, force: true });
          }
        });
      });
    });

    describe("remove", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("FileHandle", () => {
    describe("[Symbol.asyncDispose]", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("appendFile", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("chmod", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("chown", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("close", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("createReadStream", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("createWriteStream", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("datasync", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("read", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("readFile", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("readLines", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("readableWebStream", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("readv", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("stat", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("sync", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("truncate", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("utimes", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("write", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writeFile", () => {
      it("TODO", () => {
        // TODO
      });
    });

    describe("writev", () => {
      it("TODO", () => {
        // TODO
      });
    });
  });

  describe("access", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("appendFile", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("chmod", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("chown", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("copyFile", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("cp", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("glob", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("lchmod", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("lchown", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("link", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("lstat", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("lutimes", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("mkdir", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("mkdtemp", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("mkdtempDisposable", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("open", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("opendir", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("readFile", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("readdir", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("readlink", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("realpath", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("rename", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("rm", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("rmdir", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("stat", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("statfs", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("symlink", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("truncate", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("unlink", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("utimes", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("watch", () => {
    it("TODO", () => {
      // TODO
    });
  });

  describe("writeFile", () => {
    it("TODO", () => {
      // TODO
    });
  });
});
