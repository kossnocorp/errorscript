# node:fs/promises > DisposableTempDir > [Symbol.asyncDispose]

- ID: `node/24/fs/promises.test.ts::["node:fs/promises","DisposableTempDir","[Symbol.asyncDispose]"]`
- Test: `pkgs/lib/tests/node/24/fs/promises.test.ts`, exact managed suite above.
- Declaration: `@types/node@24.19.2/fs/promises.d.ts:1030-1043`: `interface DisposableTempDir extends AsyncDisposable`, with the sole signature `[Symbol.asyncDispose](): Promise<void>`. The inherited `AsyncDisposable` signature is `[Symbol.asyncDispose](): PromiseLike<void>` (TypeScript 7.0.2, `lib.esnext.disposable.d.ts:37-38`), not another built-in implementation. The trailing `declare module "node:fs/promises"` re-exports `"fs/promises"`. `mkdtempDisposable(prefix: PathLike, options?: EncodingOption): Promise<DisposableTempDir>` is the factory at line 1063. The FileHandle method at line 544 is unrelated.
- Runtime mapping: the async method on the null-prototype object returned by `mkdtempDisposable`, closing over `remove` and a resolved pathname. It does **not** dynamically call `this.remove`.
- Status: complete for the required Node 24.19.0 Linux runtime; Windows branches inspected in source, not executed.
- Contract candidate: `Returns<Promise<void>, Error | TypeError>` for the native method returned by the factory. Permission-model callback throws are a separate delivery effect described below, not a synchronous throw or promise rejection of the initiating call. Node annotation delivery is deferred; this audit changes no annotations.

## Conclusion

The method returns a promise. Ordinary filesystem failures reject it with exact native `Error` (OS-dependent codes, including the reproduced `ENOTDIR` and `EACCES`). A **successfully created** disposable with an ASCII-encoded pathname containing NUL rejects on disposal with `TypeError`, code `ERR_INVALID_ARG_VALUE`. This is not a factory failure: the factory has already resolved before the tested invocation.

Node's permission model also produces `Error`, code `ERR_ACCESS_DENIED`. A readdir/unlink permission error reaches the internal completion callback and becomes a rejection. In contrast, native `rmdir` checks permission with the synchronous throw macro, even in callback mode. When entered from the asynchronous lstat callback, this escapes that callback, is delivered as `uncaughtException`, and leaves the disposal promise pending. This branch was verified in a disposable subprocess; it must not be described as `.rejects`.

There is no receiver check, no argument conversion, and no invoked supplied callback/getter/proxy/iterator. Borrowing the method with a hostile proxy receiver and argument does not access either. The captured string is immutable; replacing the public `path` or `remove` property cannot change it. No arbitrary supplied-value propagation or `Throws<F>` term is warranted for this native implementation. Structurally implementing the TypeScript interface with one's own method is outside this built-in contract.

Scope assumes unmodified built-ins/prototypes, normal process operation and available memory. Engine termination, allocation exhaustion, injected async hooks and arbitrary monkey-patching are excluded, not classified as application errors. No privileges, network, filesystem emulation or timing races are used. POSIX permission repros require an ordinary non-root user and a filesystem honoring mode bits; the verified runtime used uid 1000.

## Proof

All Node links below are pinned to **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**. The local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, so source was read with `git show <pinned-revision>:<path>`, not from HEAD. [`src/node_version.h:25-32`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L32) declares major 24, minor 19, patch 0, release build; this matches the tested binary.

### Mapping and closure boundaries

- [`lib/fs/promises.js:3`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs/promises.js#L3): `module.exports = require('internal/fs/promises').exports;`.
- [`lib/internal/fs/promises.js:1261-1291`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/promises.js#L1261-L1291): `const fullPath = pathModule.resolve(cwd, path);`, `const remove = async () => {`, and `async [SymbolAsyncDispose]() { await remove(); }`. The fixed rimraf options are `{ maxRetries: 0, recursive: true, retryDelay: 0 }`. Initial prefix/options validation, encoding errors and `process.cwd()` failures precede creation of the object and do not belong to this method. The method itself uses neither `this` nor arguments. [`:143`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/promises.js#L143) maps `lazyRimRaf` to `require('internal/fs/rimraf').rimrafPromises`.

### Every rimraf route and native errors

[`lib/internal/fs/rimraf.js:27-167`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/rimraf.js#L27-L167):

1. `rimrafPromises` constructs a primordial Promise and calls `rimraf`; its callback says `if (err) return reject(err);`, otherwise `resolve()`. It preserves the native error object rather than wrapping it.
2. `_rimraf` lstats the pathname. `ENOENT` succeeds; a directory goes to `_rmdir`. Otherwise it attempts unlink, including after non-ENOENT lstat errors. Unlink `ENOENT` succeeds, `EISDIR` calls `_rmdir`, `EPERM` calls the platform handler, other errors are delivered unchanged. Thus the ancestor-file repro observes the **unlink** error, not the earlier lstat error.
3. `_rmdir` calls rmdir; `ENOTEMPTY`/`EEXIST` (and POSIX `EPERM`) recurse into `_rmchildren`; `ENOTDIR` returns `originalErr`. Other errors are delivered. `_rmchildren` reads entries as raw buffers, recurses over children, returns the first failure, and finally rmdirs the parent. Empty entry lists go straight to rmdir. Raw filenames cannot contain NUL or directory separators; the generated child buffers and fixed callbacks are not user-controlled JS objects.
4. The top-level retry condition is `retries < options.maxRetries`; maxRetries is zero here, so delayed retries never run. It suppresses a final `ENOENT` (`err = null`), including races with external deletion. Changing files concurrently can change the selected OS code, not the error constructor.
5. Windows `fixWinEPERM` tries chmod `0o666`, then stat, then rmdir/unlink. chmod/stat failures return the original error unless `ENOENT`, which succeeds. Its final unlink returns its own error. These use the same native error builders. No Windows-specific `TypeError`, `SyntaxError` or `RangeError` is introduced. Windows execution is not claimed.

[`lib/fs.js:1181-1214,1574-1607,1674-1689,1988-1993,2091-2098`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs.js#L1181-L1214) supplies the callback wrappers, validated paths, and native bindings. All callbacks are internal functions; rmdir options are undefined; readdir encoding is the constant `'buffer'`; stat options and chmod mode are internal constants. [`lib/internal/fs/utils.js:320-339,702-728,884-897`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/utils.js#L702-L728) validates these inputs. Type/option/callback validation is therefore satisfied except the captured-path NUL case below. Filesystem path-size errors (e.g. `ENAMETOOLONG`) remain OS `Error`s, not JS numeric-validation `RangeError`s.

[`src/node_file.cc:700-702,761-780`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_file.cc#L761-L780) creates `UVException(...)` from a failed filesystem result and calls `wrap->Reject(exception)`; `FSReqCallback::Reject` invokes `oncomplete`. The relevant binding routes are lstat `:1172-1191`, unlink `:1591-1612`, rmdir `:1625-1643`, readdir `:2022-2077`, chmod `:2860-2880`, and stat `:1102-1144`. [`AfterScanDir :921-955`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_file.cc#L921-L955) also returns `UVException` for enumeration errors; buffer encoding of filenames avoids text-decoding/URI validation. Allocation failures are outside the scope stated above.

[`src/api/exceptions.cc:91-151`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/api/exceptions.cc#L91-L151) proves the exact base constructor, not merely inheritance: `Exception::Error(js_msg)`. It adds `errno`, `code`, `syscall` and paths. There is no code-dependent switch to `SyntaxError` or another constructor.

### Reachable TypeError despite an argumentless method

[`src/node_file.cc:878-895`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_file.cc#L878-L895) resolves mkdtemp's path through `StringBytes::Encode(... req->path, req_wrap->encoding())`. [`src/string_bytes.cc:563-572`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/string_bytes.cc#L563-L572) handles ASCII with `nbytes::ForceAscii`; [`deps/nbytes/src/nbytes.cpp:189-192`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/nbytes/src/nbytes.cpp#L189-L192) says `dst[i] = src[i] & 0x7f;`. U+0100's UTF-8 bytes C4 80 become 44 00. The actual directory exists with its original UTF-8 name, but the returned string contains NUL.

[`lib/path.js:92-163,1245-1286`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/path.js#L1245-L1286) validates strings and normalizes separators/dot segments, not NULs. The factory therefore captures the NUL-containing resolved string and successfully returns the disposable.

On disposal, lstat calls `getValidatedPath`; [`lib/internal/fs/utils.js:702-728`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/fs/utils.js#L702-L728) tests `!StringPrototypeIncludes(path, '\u0000')` and otherwise throws `new ERR_INVALID_ARG_VALUE.HideStackFramesError(...)`, with reason `'must be a string, Uint8Array, or URL without null bytes'`. [`lib/internal/errors.js:1460-1467`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1460-L1467) defines that error with default base `TypeError` (the optional RangeError constructor is not selected here). This throw occurs inside rimrafPromises' executor and becomes its rejection, propagated through both async wrappers. The test verifies `.constructor === TypeError` and the stable code. A `'buffer'` result rejected by `path.resolve` at factory time is deliberately not counted here.

### Permission model and delivery channels

- [`src/node_file.cc:3159-3169`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_file.cc#L3159-L3169) checks mkdtemp write permission on the template ending in literal `XXXXXX`. Granting that exact template allows successful factory creation without granting disposal's randomly suffixed path. This isolates a real disposal failure, not a fixture failure.
- [`lib/fs.js:1681-1684`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/fs.js#L1681-L1684) delivers an lstat read-permission failure through its callback. Rimraf then attempts unlink. Native unlink/readdir use `ASYNC_THROW_IF_INSUFFICIENT_PERMISSIONS` (`src/node_file.cc:1604-1608,2061-2065`); this calls `req_wrap->Reject(err)` via [`src/permission/permission.cc:155-162`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/permission/permission.cc#L155-L162). On a nonempty directory with write permission and no read permission, unlink/rmdir lead to readdir and its read denial becomes a promise rejection.
- Native rmdir instead uses `THROW_IF_INSUFFICIENT_PERMISSIONS` before either async/sync branch (`src/node_file.cc:1634-1635`). [`src/permission/permission.h:30-55`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/permission/permission.h#L30-L55) and [`permission.cc:144-162`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/permission/permission.cc#L144-L162) distinguish `env->isolate()->ThrowException(err)` from the callback path. No try/catch surrounds `_rmdir` inside the lstat callback. A successful asynchronous lstat followed by denied rmdir thus throws from that callback, not into the already-returned Promise executor. The Windows chmod branch also uses this synchronous macro (`src/node_file.cc:2869-2870`); if reached asynchronously, its denial similarly escapes the internal callback. No user callback error is being propagated.
- [`src/permission/permission.cc:121-141`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/permission/permission.cc#L121-L141) constructs `ERR_ACCESS_DENIED` with permission/resource fields; [`src/node_errors.h:44`](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h#L44) specifies `V(ERR_ACCESS_DENIED, Error)`. The JS lstat error definition likewise uses Error (`lib/internal/errors.js:1116-1122`).

### Promise semantics

ECMA-262 clone revision **`40ce6f206b5669e69bf010a38267ae24a2843404`**, [`spec.html`, Promise constructor, AsyncBlockStart and Await](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html), anchors [`sec-promise-executor`](https://tc39.es/ecma262/#sec-promise-executor), [`sec-asyncblockstart`](https://tc39.es/ecma262/#sec-asyncblockstart), [`await`](https://tc39.es/ecma262/#await). Executor abrupt completion calls the reject function; AsyncBlockStart's throw case says `Call(_promiseCapability_.[[Reject]], *undefined*, « _result_.[[Value]] »)`. Await resumes with the rejection as a throw completion. These algorithms explain why the NUL validation throw and internal callback rejection become disposal rejections without wrapping; they do not catch later event-loop callback throws.

## Repros and validation

All tests are in the granted suite and tagged `node-only`. Each nested scenario has a separate `it("throws Error", ...)` or `it("throws TypeError", ...)`:

| Scenario | Observed effect |
| --- | --- |
| captured ASCII-encoded path contains NUL | awaited rejection, exact TypeError, `ERR_INVALID_ARG_VALUE` |
| ancestor replaced by a file | awaited rejection, exact Error, `ENOTDIR`, `unlink`, captured path; hostile receiver/argument not accessed |
| parent is not writable | awaited rejection, exact Error, `EACCES`, `rmdir` |
| recursive child is not readable | awaited rejection, exact Error, `EACCES`, `scandir`, locked child's path |
| permission-model readdir denial | isolated Node subprocess awaits `assert.rejects`, exact Error, `ERR_ACCESS_DENIED`, `FileSystemRead`, captured path |
| permission-model rmdir denial escapes its callback | isolated Node subprocess asserts exact Error, `ERR_ACCESS_DENIED`, `FileSystemWrite`, captured path and `uncaughtException` origin; disposal promise remains unsettled |

The child scripts install no mocks or polyfills. The uncaughtException listener is installed **after** successful factory creation; it asserts the origin to reject an unhandled-rejection false positive. The parent checks a success marker and subprocess exit status. Children use `process.execPath`, five-second timeouts, and normal process shutdown. Each fixture is under its own mkdtemp root; finally blocks restore permissions and recursively remove the real paths, including the NUL-encoding fixture whose public path cannot be used for cleanup. No global cwd or parent-process permission state is modified.

Commands from the repository root (each prefixed with `export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`):

```sh
mise exec -- node -p 'JSON.stringify(process.versions)'
mise run //pkgs/lib:test/ts tests/node/24/fs/promises.test.ts --project node-24 \
  -t '^node:fs/promises > DisposableTempDir > \[Symbol\.asyncDispose\] > [^>]+ > throws Error$' --maxWorkers 1
mise run //pkgs/lib:test/ts tests/node/24/fs/promises.test.ts --project node-24 \
  -t '^node:fs/promises > DisposableTempDir > \[Symbol\.asyncDispose\] > [^>]+ > throws (Error|TypeError)$' --maxWorkers 1
```

Results: initial Error-only run **5 passed, 53 skipped, 1 file passed**; final expanded run **6 passed, 53 skipped, 1 file passed**, Vitest **5.0.5**. All six expected repros executed; skipped tests are other inventory callables. Node **24.19.0**, V8 **13.6.233.17-node.51**, libuv **1.52.1**, Linux uid **1000**. Plain shell Node was 22.23.1 and was **not** used to validate this callable; mise supplied the matching Node 24 runtime. Chromium does not include Node inventory files and is not a required project here.

Ran `mise exec -- pnpm exec oxfmt pkgs/lib/tests/node/24/fs/promises.test.ts` and `git diff --check -- pkgs/lib/tests/node/24/fs/promises.test.ts` successfully, then repeated the final six-test command: **6 passed, 53 skipped, 1 file passed**. Diff review confirmed only the selected suite and its necessary `expect` import changed.

## Open questions

No remaining intrinsic-error proof blocker for the required runtime. No shared configuration change is needed. Windows recovery branches were source-audited only; these POSIX mode-bit fixtures are not claimed portable to Windows or root. Annotation delivery remains deferred to the coordinator. The permission-model uncaught callback effect must remain documented separately rather than being presented as a rejection guarantee.
