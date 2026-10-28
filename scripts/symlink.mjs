import { spawnSync } from 'node:child_process';
import { lstat, mkdir, symlink, unlink } from 'node:fs/promises';
import { homedir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const compiler = fileURLToPath(new URL('../subs/errs/', import.meta.url));
const metadata = spawnSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
  cwd: compiler,
  encoding: 'utf8',
});
if (metadata.error) throw metadata.error;
if (metadata.status !== 0) throw new Error(metadata.stderr);

const suffix = process.platform === 'win32' ? '.exe' : '';
const target = path.join(JSON.parse(metadata.stdout).target_directory, 'debug', `tsrs${suffix}`);
const link = path.join(homedir(), '.local', 'bin', `errs${suffix}`);
await lstat(target);
await mkdir(path.dirname(link), { recursive: true });
const existing = await lstat(link).catch(error => {
  if (error.code !== 'ENOENT') throw error;
});
if (existing) {
  if (!existing.isSymbolicLink()) throw new Error(`Cannot replace existing file: ${link}`);
  await unlink(link);
}
await symlink(target, link, 'file');
console.log(`${link} -> ${target}`);
