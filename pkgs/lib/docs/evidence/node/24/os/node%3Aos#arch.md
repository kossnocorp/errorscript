# node:os > arch

- ID: `node/24/os.test.ts::["node:os","arch"]`
- Test: `pkgs/lib/tests/node/24/os.test.ts`, managed `node:os > arch`.
- Declaration: installed `@types/node` **24.19.2**, `os.d.ts:437`: `function arch(): NodeJS.Architecture;` (one overload). `os.d.ts:10` declares `"os"`; lines 505–507 re-export it from `"node:os"`. `process.d.ts:273–283` defines the architecture string union. No callback/interface implementation ambiguity.
- Runtime mapping: `node:os.arch` exports the ordinary JavaScript `arch` function in `lib/os.js:165–167,311–312`.
- Status: complete.
- Contract candidate: `Returns<NodeJS.Architecture, never>` (equivalently no `Throws` effect). Candidate only; Node annotation delivery is deferred, not installed.

## Conclusion

No ordinary synchronous intrinsic errors or propagated effects. No promise, iterator, callback, or event channel. The function returns an existing own data property of the captured Node process object. It neither reads its receiver nor examines arguments, including extra arguments, proxies, coercion hooks, or supplied functions. Invalid receivers require no validation and cannot trigger a getter/trap through this body.

Scope: genuine built-in in an initialized, unmodified Node 24.19.0 environment. Excludes module-loading failures, argument/callee evaluation outside the call, OOM/process termination, and arbitrary monkey-patching. In particular, `process.arch` is configurable: deliberately replacing it with a throwing getter would introduce arbitrary synchronous propagation, but is not the stock callable contract. There are no platform-dependent algorithm branches; only the compile-time architecture string differs. Construction is not a declared overload; this audit covers calling the function, not surrounding `new`/`Reflect.construct` machinery. `arch[Symbol.toPrimitive]` is a separate function, not invoked by calling `arch`.

## Proof

Node source revision: **`cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`**, exact tag **v24.19.0**. Local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; all Node excerpts were obtained with `git show`/`git grep` at the pinned revision, not HEAD. [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines major/minor/patch as 24/19/0. The installed runtime's embedded `os` source SHA-256 matches that revision byte-for-byte (validation below).

1. **Full callable and export:** [lib/os.js:162–168](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/os.js#L162-L168) reads verbatim `function arch() {` / `return process.arch;` / `}`. [Lines 311–312](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/os.js#L311-L312) export `arch` directly. No native OS query, checked wrapper, validator, conversion, or nested callable is reachable. `getCheckedFunction` and its `ERR_SYSTEM_ERROR` at lines 63–84 wrap other exports, not `arch`. The sole expression does not reference `this` or arguments. Strict mode at line 22 also prevents implicit receiver boxing, although the receiver is unused regardless.
2. **Captured process, not a caller getter:** [src/builtin_info.h:41–47](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/builtin_info.h#L41-L47) specifies the normal built-in parameters `"exports", "require", "module", "process", "internalBinding", "primordials"`; [src/node_builtins.cc:364–378](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_builtins.cc#L364-L378) passes those parameters to `ScriptCompiler::CompileFunction`. [lib/internal/bootstrap/realm.js:395–398](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/internal/bootstrap/realm.js#L395-L398) invokes `fn(this.exports, requireFn, this, process, internalBinding, primordials);`. Thus `process` is an initialized lexical parameter captured by `arch`, not a receiver field or fresh global-property lookup.
3. **Property is data, not an accessor:** [src/node_process_object.cc:128–129](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_process_object.cc#L128-L129) initializes `READONLY_STRING_PROPERTY(process, "arch", per_process::metadata.arch);`. [src/util.h:739–744,762–763](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/util.h#L739-L763) expands this to `DefineOwnProperty(..., value, v8::ReadOnly)` with the converted string value, not an accessor callback. [src/node_metadata.cc:205](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_metadata.cc#L205) says `Metadata::Metadata() : arch(NODE_ARCH), platform(NODE_PLATFORM) {}`; [src/node_metadata.h:146](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_metadata.h#L146) declares `const std::string arch;`. Initialization and string allocation happen before this call; they are not per-call OS failure paths.
4. **Completion of the only expression:** ECMA-262 checkout revision **`40ce6f206b5669e69bf010a38267ae24a2843404`**, [OrdinaryGet, `sec-ordinaryget`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L13337-L13360): “If IsDataDescriptor(_propertyDesc_) is *true*, return _propertyDesc_.[[Value]].” The established own data descriptor avoids both prototype traversal and accessor invocation. [Return evaluation, `sec-return-statement-runtime-semantics-evaluation`](https://github.com/tc39/ecma262/blob/40ce6f206b5669e69bf010a38267ae24a2843404/spec.html#L23322-L23336) ends “Return ReturnCompletion(_exprValue_).” This is an ordinary synchronous function, so no async awaiting path applies. These language semantics explain why the complete Node body has no ordinary abrupt path; the conclusion is not inferred from successful samples.

## Repros and validation

Required project: **node-24** only; Chromium does not run Node inventory files. Test body is exactly `it.skip("never throws")`, a proof marker, not an empirical repro or a passing assertion.

With `export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`:

```sh
mise run //pkgs/lib:test/node tests/node/24/os.test.ts \
  -t '^node:os > arch > never throws$' --maxWorkers 1
```

Exit 0, Vitest **5.0.5**: **1 file skipped, 20 tests skipped, 0 executed repros**. The full-suite anchored filter selects the single reviewed skip marker; the other 19 callable TODOs are excluded. This skipped run validates collection, not absence of errors. No concrete error repros are expected for this conclusion.

Runtime verification: `mise which node` resolved `/home/koss/.local/share/mise/installs/node/24.19.0/bin/node`; `mise exec -- node -p 'JSON.stringify(process.versions)'` reported Node **24.19.0**, V8 **13.6.233.17-node.51**. Bare `node --version` outside mise reported 22.23.1 and was not used to validate the callable.

Additional standalone smoke/source-identity check (exit 0; not a replacement for the proof or a Vitest repro):

```sh
mise exec -- node --input-type=module <<'JS'
import assert from 'node:assert/strict';
import { arch } from 'node:os';
import { createHash } from 'node:crypto';
const descriptor = Object.getOwnPropertyDescriptor(process, 'arch');
assert.equal(typeof descriptor.value, 'string');
assert.equal(descriptor.get, undefined);
const { proxy, revoke } = Proxy.revocable({}, {});
revoke();
let touched = 0;
const hostile = new Proxy({}, { get() { touched++; throw 'unexpected access'; } });
const receivers = [undefined, null, 0, Symbol(), hostile, proxy];
for (const receiver of receivers) {
  assert.equal(Reflect.apply(arch, receiver, [hostile, proxy, Symbol(), 1n, () => { throw 'unexpected callback'; }]), descriptor.value);
}
assert.equal(touched, 0);
console.log(JSON.stringify({node: process.version, v8: process.versions.v8, platform: process.platform, arch: process.arch, descriptor, receiverChecks: receivers.length, osSourceSHA256: createHash('sha256').update(process.binding('natives').os).digest('hex')}));
JS
git -C ~/code/clones/node show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/os.js | sha256sum
```

Observed linux/x64, descriptor `{value:"x64",writable:false,enumerable:true,configurable:true}`, six receiver checks, zero trap accesses. Both embedded and pinned source hash: `5b14b21284e19382708f588ff7fa5a212c5335a84b1d9ae6ebdf626244f8fde9`. No global state was modified and no external resources needed teardown. No mocks/polyfills.

## Open questions

None for this scoped callable proof. Annotation integration remains the coordinator's deferred Node-delivery task; no annotation table was changed.
