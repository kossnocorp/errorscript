# node:querystring > ParseOptions > decodeURIComponent

- ID: `node/24/querystring.test.ts::["node:querystring","ParseOptions","decodeURIComponent"]`
- Test/declaration: `pkgs/lib/tests/node/24/querystring.test.ts:4–10`; `@types/node@24.19.2`, `querystring.d.ts:22–33,150–152`. One optional callable-property signature: `decodeURIComponent?: ((str: string) => string) | undefined`; no overloads or additional merged declarations found in that package.
- Runtime mapping: none unique. This is a caller-supplied option callback, not an exported Node built-in or the global `decodeURIComponent`.
- Status: blocked
- Contract candidate: only a parametric `(str: string) => Returns<string, E_decoder>` for a supplied decoder; without knowledge of that implementation, `Returns<string, unknown>`. This is not an intrinsic built-in error list or an annotation of `querystring.parse`. Node annotation delivery remains deferred.

## Conclusion

The declaration does not designate an implementation whose error set can be audited. A supplied decoder can synchronously throw any JavaScript value; no concrete intrinsic error constructor can be assigned to this inventory entry. Its receiver/argument validation, conversions, getters, proxies, nested calls, and other effects depend on that implementation. Neither `URIError` (from the global decoder) nor `never` (inferred from a default decoder's fallback) is justified here.

The consumer calls the decoder synchronously and catches its immediate thrown completion, falling back to `QueryString.unescape`. Direct callback throws therefore are not automatically propagated out of `parse`. Option-property getter/proxy failures occur during consumer selection outside that catch; they are effects of `parse`, not proof of this callback's intrinsic contract. The declared callback returns `string`; no promise, iteration, or event delivery channel is specified. User implementations violating the signature or scheduling later work cannot be assigned a fixed effect set from this type.

The managed literal TODO remains unchanged. No invented callback or default implementation has been substituted as the callable under audit.

## Proof

### Declaration and module alias

Versioned primary declaration: [@types/node@24.19.2/querystring.d.ts](https://unpkg.com/@types/node@24.19.2/querystring.d.ts), locally SHA-256 `b36beffbf8acdc3ebc58c8bb4b75574b31a2169869c70fc03f82895b93950a12`.

- Lines 28–32 say “The function to use when decoding percent-encoded characters in the query string”, specify `@default` as `querystring.unescape()`, and declare `decodeURIComponent?: ((str: string) => string) | undefined;`. A default for an option is not a unique implementation of that optional callback property.
- Lines 150–152 declare `module "node:querystring"` with `export * from "querystring";`. The managed namespace resolves to the same interface in `module "querystring"`, not a distinct decoder implementation.
- Line 114 declares `function parse(str: string, sep?: string, eq?: string, options?: ParseOptions): ParsedUrlQuery;`. Line 122 declares `const decode: typeof parse;`. These are the parser and its alias, not aliases of the selected callback property.

### Matching Node implementation and consumer boundary

All Node citations below are pinned to `cdc1b38d40cb567b7ad0b39c86addf830a0af0ae` (release commit “2026-08-03, Version 24.19.0 'Krypton' (LTS)”). Local clone HEAD was `510d84c330e8e7d2bb8699608163024baf54eaa6`; it was **not** used as the runtime source. Files were read with `git show` at the pinned revision.

1. [src/node_version.h:25–27](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/src/node_version.h#L25-L27) defines `NODE_MAJOR_VERSION 24`, `NODE_MINOR_VERSION 19`, `NODE_PATCH_VERSION 0`, matching the mise runtime.
2. [lib/querystring.js:45–58](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L45-L58) exports `unescape: qsUnescape`, `parse`, and `decode: parse`. It exports no `ParseOptions` constructor or `decodeURIComponent` callable. `ParseOptions` is an interface only.
3. [lib/querystring.js:340–344](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L340-L344): `let decode = QueryString.unescape;` followed by `if (options && typeof options.decodeURIComponent === 'function') { decode = options.decodeURIComponent; }`. The property supplies an arbitrary function; omission/non-function input selects a different callable. Property reads are outside `decodeStr`'s catch (including the second read if an accessor changes its result).
4. [lib/querystring.js:286–290](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L286-L290) invokes `decodeStr(key, decode)` / `decodeStr(value, decode)` for nonempty encoded components. The parser reaches this helper for completed pairs at [line 380](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L380) and its trailing pair at [line 464](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L464). These are consumer invocation paths, not callback bodies.
5. [lib/querystring.js:478–483](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L478-L483): `try { return decoder(s); } catch { return QueryString.unescape(s, true); }`. The immediate call is synchronous and its thrown value is caught without filtering its type. Testing a throwing supplied function through `parse` would test this catch/fallback boundary, not establish an intrinsic callback error.
6. [lib/querystring.js:26–37](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L26-L37) obtains `decodeURIComponent` from `primordials`; [lines 131–136](https://github.com/nodejs/node/blob/cdc1b38d40cb567b7ad0b39c86addf830a0af0ae/lib/querystring.js#L131-L136) implement a separate `qsUnescape`: `try { return decodeURIComponent(s); } catch { return QueryString.unescapeBuffer(s, decodeSpaces).toString(); }`. Neither the primordial decoder nor the fallback is identical to the arbitrary optional callback declared in `ParseOptions`.

This mapping blocker arises before an exhaustive algorithm/error audit can be performed for the selected callback. These consumer citations establish its boundary only; they do not claim a complete error contract for `parse`, `unescape`, or the global decoder.

## Repros and validation

- Read `pkgs/lib/docs/auditing.md`; located the managed suite and corresponding package declarations with targeted reads and `rg -n 'ParseOptions|decodeURIComponent' node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node --glob '*.d.ts'`.
- `readlink -f pkgs/lib/node_modules/@types/node` resolved to the installed `@types+node@24.19.2` package; package metadata reports `24.19.2`. `sha256sum node_modules/.pnpm/@types+node@24.19.2/node_modules/@types/node/querystring.d.ts` produced the digest above.
- With `PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"`, plain `node --version` reported `v22.23.1`; `mise exec -- node --version` reported **`v24.19.0`**. No behavior was tested under the plain-PATH Node.
- Source commands in `~/code/clones/node`: `git rev-parse HEAD`, `git show -s --format='%H %s' cdc1b38d40cb567b7ad0b39c86addf830a0af0ae`, and `git show cdc1b38d40cb567b7ad0b39c86addf830a0af0ae:lib/querystring.js` / `:src/node_version.h`, with numbered targeted ranges.
- No Vitest command executed: **0 repros** in `node-24`; Chromium is not applicable to this Node-only inventory entry. Running the retained empty TODO would not validate an implementation. No precise-constructor or propagation repro is claimed.

## Open questions

The coordinator must decide how to represent declaration-only callback effects in the inventory/annotation model. A specific supplied implementation and its effect parameter would be needed for behavioral validation; selecting a default would change the audited callable. No runner, dependency, or configuration change resolves this identity blocker. Keep the callable pending; no declaration annotations or shared files were changed.
