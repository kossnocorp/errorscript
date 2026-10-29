# node:buffer > Blob > arrayBuffer

- ID: `node/24/buffer.test.ts::["node:buffer","Blob","arrayBuffer"]`
- Test: `pkgs/lib/tests/node/24/buffer.test.ts`, exact managed path above.
- Declaration: `@types/node@24.19.2`, `buffer.d.ts:175`, sole overload
  `arrayBuffer(): Promise<ArrayBuffer>` in `module "buffer"` / `class Blob`.
  Lines 1939–1941 re-export it from `node:buffer`; the conditional global Blob
  alias (lines 3, 1921–1928) and `File extends Blob` (230) do not introduce another
  implementation or overload of this method.
- Runtime mapping: `node:buffer` lazily exports `internal/blob.Blob`; the method
  calls the private JS `arrayBuffer(blob)` helper and native Blob/DataQueue reader.
- Status: complete (scoped assumptions below; annotation delivery deferred).
- Contract candidate: ordinary genuine receiver, including invalid receiver
  validation: `Returns<Promise<ArrayBuffer>, TypeError | DOMException>`.
  With unrestricted receivers/proxies: `Returns<Promise<ArrayBuffer>, unknown>`;
  more precisely an error component `TypeError | DOMException | ReceiverGetEffect`, where
  `ReceiverGetEffect` can be any JS value. These are candidate effect contracts,
  not installed declaration annotations. Channels are separated below.

## Conclusion

| Channel | Effect |
| --- | --- |
| Promise rejection | `TypeError`, code `ERR_INVALID_THIS`, for a receiver without the internal handle. |
| Promise rejection | `DOMException`, name `NotReadableError`, code `0`, for failed file-backed reads, including backing-file modification or removal. OS errors are mapped to this exception, not exposed as ordinary `Error` objects. |
| Synchronous | `TypeError` from revoked receiver proxy / invalid proxy operations during the handle lookup. |
| Synchronous propagation | Arbitrary values from receiver property access (getters, proxy traps and trap lookup). Both the validation lookup and the second handle lookup can throw before the promise is returned. Supplied `Error` and `SyntaxError` examples are propagation, not intrinsic production. |

There are no explicit arguments to convert and no caller-supplied callback,
iterator, or stream method invoked for a genuine Blob. In particular the
implementation does not invoke an overridden public `stream()` or `arrayBuffer()`
method when entered via the original prototype method. Native completion callbacks
belong to this implementation and settle its promise; they are not user events or
iteration-time errors. Empty, memory-backed, sliced, and composite blobs traverse
the same reader/concatenation path. A memory-only Blob does not acquire a file-read
failure path merely from its content.

Assumptions: normal Node runtime, intact intrinsics/internal Blob handles, no global
prototype monkey-patching or failing async-hooks instrumentation. Hostile receiver
property access is explicitly included, but fabricated native handles and replacement
internal readers are not a supported Blob implementation. Resource exhaustion/OOM,
process termination, and engine bugs are excluded per the auditing policy. In
particular native concatenation has an allocation-failure `RangeError` branch,
documented below rather than silently claimed impossible. No allocation-failure
contract or unsafe OOM repro is asserted. No browser portability claim is made.

## Proof

### Version and implementation identity

All Node links below pin revision
`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae` (`v24.19.0^{commit}`). The research clone's
HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`, so source was read with
`git show cdc1b38:...`, not from its mismatched working tree.
[node_version.h](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27)
defines version 24.19.0;
[v8-version.h](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/deps/v8/include/v8-version.h#L11-L14)
defines V8 13.6.233.17, matching the tested Node's V8
`13.6.233.17-node.51`.

[lib/buffer.js:1454–1458](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/buffer.js#L1454-L1458)
exports `['Blob', 'resolveObjectURL']` from `'internal/blob'`. This is a real
built-in method, not a callback declaration.

### Validation and synchronous property-access effects

[lib/internal/blob.js:112–114, 294–299](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/blob.js#L294-L299):
`isBlob` is `return object?.[kHandle] !== undefined;` and the method says
`return PromiseReject(new ERR_INVALID_THIS('Blob'));` if validation fails,
otherwise `return arrayBuffer(this);`. It is not an async function and has no
try/catch. Nullish/ordinary missing-handle receivers therefore reject, while an
abrupt property access escapes synchronously. No argument coercion occurs.

[lib/internal/errors.js:1548](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L1548)
registers `E('ERR_INVALID_THIS', 'Value of "this" must be of type %s', TypeError, HideStackFramesError);`.
The generated NodeError subclass exposes `get ['constructor']() { return Base; }`
at [lines 511–515](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/errors.js#L511-L515).
The tests check the exact exposed constructor and the stable Node code, not just
membership in the broad Error inheritance chain.

ECMA-262 source revision `40ce6f206b5669e69bf010a38267ae24a2843404`:
[Proxy [[Get]]](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16292-L16318),
anchor `sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver`, says
`Perform ? ValidateNonRevokedProxy(_obj_).`, `Let _trap_ be ? GetMethod(...)`, and
`Let _trapResult_ be ? Call(_trap_, ...)`. Thus trap lookup, invocation, target
lookup and proxy invariant checks may abruptly complete, with no translation by
Node. [ValidateNonRevokedProxy](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L16551-L16565)
says `If _proxy_.[[ProxyTarget]] is *null*, throw a *TypeError* exception.`
Ordinary accessor lookup is covered by
[`sec-ordinaryget`, lines 13337–13358](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13337-L13358)
in the same pinned specification: `Return ? Call(_getter_, _receiver_).` Arbitrary getter/trap throws
are therefore not bounded by a finite set of built-in error constructors.

### Reader, file errors, and settlement

[lib/internal/blob.js:434–460](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/blob.js#L434-L460):
after `PromiseWithResolvers()`, the helper reads
`const reader = blob[kHandle].getReader();`. This second receiver lookup is still
outside any rejection wrapper. It defines `readNext`, calls it, then returns the
promise. The internal pull callback has three branches:

- `status === 0`: `resolve(concat(buffers));`.
- `status < 0`: constructs
  `lazyDOMException('The blob could not be read', 'NotReadableError')`, then
  `reject(error);`.
- Otherwise: accumulates a provided buffer and `queueMicrotask(() => readNext());`.

The closure passed to `queueMicrotask` is always a function; accumulated buffers
and the result are native-created values, not caller-provided coercion hooks.
Promise construction and ordinary successful resolution add no validation error.

[src/node_blob.cc:262–270, 301–339, 342–411](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_blob.cc#L262-L411)
creates a reader from `strong_ptr->data_queue_->get_reader()`, then calls
`reader->inner_->Pull(...)`. Its callback copies supplied bytes into a new
ArrayBuffer and invokes the JS callback with status/data; a no-data callback uses
`Int32::New(env->isolate(), status)`, preserving negative status values. Native
unwrap/function assertions are satisfied by genuine handles and the built-in
callback; they are not an additional public receiver validation route.

[src/dataqueue/queue.cc:224–341](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/dataqueue/queue.cc#L224-L341):
empty/exhausted queues report `STATUS_EOS`; if a part's reader cannot be obtained,
`std::move(next)(UV_EINVAL, nullptr, 0, ...)` is delivered. Other part statuses are
forwarded; intermediate EOS advances to the next entry. Empty and memory entries
([540–635](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/dataqueue/queue.cc#L540-L635))
only provide CONTINUE/EOS. Nested DataQueue entries delegate to `inner_->Pull`
([776–790](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/dataqueue/queue.cc#L776-L790));
nesting does not change error mapping.

[File entries:882–1005](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/dataqueue/queue.cc#L882-L1005):
`is_modified` compares size and modification-time nanoseconds. `CheckModified`
also treats failed `uv_fs_fstat` as modification. Reader creation calls
`uv_fs_open` and returns null for `file < 0 || FdEntry::CheckModified(entry, file)`.
This proves both deterministic test conditions without relying on filesystem
timestamp precision: a changed size and a missing file. Checks before pulls and
after reads deliver `UV_EINVAL` on modification. Other negative `nread` values
are forwarded, except `UV_EOF`, which becomes EOS. FileHandle's actual
[read path, node_file.cc:545–660](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_file.cc#L545-L660)
dispatches `uv_fs_read`, passes its result to `EmitRead(result, buffer)`, and
normalizes zero-byte reads to EOF. No system-error Error object is constructed in
this path. Libuv/platform differences in negative error codes all reach the same
JS rejection mapping. This is not an `openAsBlob()` fixture-creation failure.

[lib/internal/util.js:684–694](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/util.js#L684-L694)
constructs `new _DOMException(message, name)` from the messaging binding's native
realm constructor. [domexception.js:71–105, 146–157](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/per_context/domexception.js#L71-L157)
sets the instance prototype to `new.target.prototype` and stores the given name;
the legacy-code getter returns `code === undefined ? 0 : code`.
`NotReadableError` is not a legacy-code entry. It is an exact DOMException, not an
ordinary Error despite its Error prototype ancestry.

### Allocation boundary

[src/node_blob.cc:48–104](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_blob.cc#L48-L104)
concatenates only internal buffers. It requests an ArrayBuffer backing store with
`BackingStoreOnFailureMode::kReturnNull`, then uses
`THROW_ERR_MEMORY_ALLOCATION_FAILED(isolate)` on failure.
[node_errors.h:109](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_errors.h#L109)
maps that code to `RangeError`. This allocation failure is excluded by the explicit
resource-exhaustion scope, not inferred absent. It occurs inside the internal
native completion callback before `resolve`, without a catch converting it to a
rejection; no general claim that every internal failure rejects is made. Other
native reader/backing-store allocations likewise fall under this exclusion.

## Mechanism review and current repros

The two synchronous propagation cases now target the real receiver's `kHandle`
symbol explicitly: first the brand lookup, then reader acquisition. Duplicate
Error/SyntaxError sentinels at the same first lookup were removed. The same source
proves arbitrary unchanged propagation. These throws are distinct from rejected
invalid-receiver TypeError and file-backing DOMException (`NotReadableError`);
those channel/condition repros remain. A Blob proxy retains its static type, so
native implementation symbols can still be receiver-effect dependencies.
Current execution count: 6; latest results are in the run checkpoint.

## Original audit validation (historical)

All eight repros have `node-only` tags. The Node inventory is excluded from the
Chromium project, so only `node-24` is required.

| Scenario / test name | Actual cases | Assertion/channel |
| --- | ---: | --- |
| `throws TypeError` | 1 | Six invalid receivers; returned Promise, awaited rejection, exact constructor and `ERR_INVALID_THIS`. |
| revoked receiver / `throws TypeError` | 1 | Synchronous exact TypeError. |
| handle lookup 1 and 2 / `throws non-Error value` | 2 | Synchronous supplied object identity, correct lookup count. |
| receiver trap / `throws Error`, `throws SyntaxError` | 2 | Separate supplied exceptions, identity and exact constructor; synchronous, not intrinsic. |
| file backing changed and removed / `throws DOMException` | 2 | Returned Promise, awaited rejection, exact constructor, name, and code. |

File fixtures are created locally before invocation and cleaned in `finally`.
Failure happens on lazy reader creation, which closes any opened descriptor;
no unclosed user-created handles, mocks, or emulation are involved.

Commands from repository root (PATH prefix on each):

```sh
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise exec -- node -p 'JSON.stringify(process.versions)'
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise run //pkgs/lib:test/ts tests/node/24/buffer.test.ts --project node-24 -t '^node:buffer > Blob > arrayBuffer > (?:.* > )?throws ' --maxWorkers 1
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" mise run //pkgs/lib:test/ts tests/node/24/buffer.test.ts --project node-24 -t '^node:buffer > Blob > arrayBuffer > (?:.* > )?throws ' --maxWorkers 1 --reporter verbose
git diff --check -- pkgs/lib/tests/node/24/buffer.test.ts 'pkgs/lib/docs/evidence/node/24/buffer/node%3Abuffer#blob#arraybuffer.md'
```

Actual runtime: Node **24.19.0**, V8 **13.6.233.17-node.51**, libuv **1.52.1**,
Linux; Vitest **5.0.5**, declarations **24.19.2**. Initial shell `node` was
22.23.1; it was used only for environment inspection, never for test validation.
Mise selected `/home/koss/.local/share/mise/installs/node/24.19.0/bin/node`.
Both test runs: **1 file passed; 8 repros passed; 98 unrelated TODO tests skipped**.
The final verbose run explicitly listed all eight repros in `node-24`; diff
whitespace validation also passed.
The filter includes the full managed suite and test boundary, plus optional local
scenario nesting; its suite names contain no regex metacharacters to escape.

## Open questions

None within the stated runtime/resource assumptions. Node external annotation
delivery is explicitly deferred to the coordinator; no installed types,
declaration annotation tables, generator, configuration, or shared helpers changed.
