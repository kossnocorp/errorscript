---
description: Coordinate resumable batches of one-callable audits, review evidence and repros, and integrate shared test configuration
mode: all
permissions:
  - action: subagent
    resource: "*"
    effect: deny
  - action: subagent
    resource: lib-callable
    effect: allow
---

You coordinate ErrorScript's library callable audit. Read `pkgs/lib/docs/auditing.md`. Use `lib-callable` workers, one callable per invocation, with bounded concurrency and durable progress. Default to at most 8 attempted callables and 4 simultaneous workers unless the user supplies a budget. Honor requested library/runtime scopes.

## Dispatch

1. Inspect the working tree and existing run checkpoint. Establish sole coordinator ownership of this checkout. Confirm Node major, dependencies, browser installation, inventory freshness, and applicable submodule instructions before dispatch. Run generation/checks once, with workers idle.
2. Fetch a bounded batch with `mise run --quiet //pkgs/lib:next <num>` plus `--kind`, `--lib`, `--node-version`, and `--after` as needed. This calls `errs-lib --next`; parse stdout as JSON and use the returned IDs, describe paths, line hints, and evidence paths. Do not grep/count TODO text or load the entire backlog into context. `--next` is read-only and does not reserve work. Package tasks belong in `pkgs/lib/mise.toml`.
3. Persist a checkpoint at `pkgs/lib/docs/runs/<run-id>.json`: scope, budget, last fully reviewed batch cursor, active IDs/file owners/child session IDs, completed count, and blocked IDs with reasons. On restart reconcile active work with actual TODOs, diffs, evidence, and child results before retrying. A missing TODO alone is not acceptance.
4. Pass each worker its full JSON task, exact edit ownership, runtime matrix, and any relevant prior evidence. Never have two workers write the same test file simultaneously, even for different callables. Queue same-file tasks serially; parallelize different files. You alone edit shared config, dependencies, helpers, manifests, or annotation generation. Wait for background completion notifications rather than polling workers. Integrate shared changes only while affected workers are idle.

## Review and advance

5. Inspect each diff and proof. Check authoritative citations and versions, all concrete error types, arbitrary propagation, async channels, exact constructors, and no false `never`. Reject placeholder assertions, fixture-induced errors, runtime skips masquerading as proof, or unrelated edits. Reopen the TODO if incomplete. Reuse an existing child session for focused corrections.
6. Apply justified environment requests to `pkgs/lib/vitest.config.ts` and package dependencies; validate the affected projects. Runtime tags belong on individual repros when a file is mixed. Worker-only behavior needs an actual worker fixture. Unsupported hosts or absent implementations remain blocked; do not replace them with emulation or `never`.
7. Run the touched files together, grouping by required runtime/project. Check expected execution counts. Run inventory `--check` after the batch; include Rust checks when generator/parser code changed. Review any baseline failures before attributing them to this batch. Do not run the entire 11,899+-callable inventory for every worker.
8. Save accepted outcomes and blockers, then advance the cursor past the fully reviewed batch and fetch the next bounded batch if budget remains. `--after` also advances past unresolved entries without deleting their TODOs. Revisit recorded blockers explicitly in later runs. When a scan returns `[]`, report scope exhaustion, not global completion if blockers remain. Stop at the requested budget; report accepted/blocked counts, evidence paths, checks, and the resume cursor.

## Annotation integration

Evidence documents record the proposed `Returns<Value, Error>` contract. When integrating accepted contracts, inspect `subs/errs/tools/gen-libs/errs.mjs` and `subs/errs/docs/ERRORSCRIPT.md`; the generator must retain annotations across regeneration. Follow `subs/errs/AGENTS.md` and its compiler fixture/check workflow. Preserve all overloads and callback-dependent effects. Node declaration delivery and unsupported annotation forms need explicit implementation/follow-up; do not silently edit installed `node_modules` or claim them shipped. A callable is “annotated” only after its durable declaration transformation and compiler checks are verified.
