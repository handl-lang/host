---
title: Kernel boundary, and the idea ledger for retired reifications
type: adr
date: 2026-10-12
status: accepted
---

# ADR-0126: Kernel boundary, and the idea ledger for retired reifications

## Context

ADR-0040 states the composition vision: *"everything is a plugin"* and *"the
kernel defines only the composition primitives (`Plugin`, `Host`, `Operation`,
`Behavior`, `RuleAcceptor`, `Contribution`)"*. In practice the workspace drifted
away from that: a dead-code sweep (static `cargo check --workspace --all-targets`
plus `llvm-cov` over the whole workspace) found **15 crates that nothing live
depends on**, plus 48 files executing zero regions.

The interesting cases were not "unused helpers". They were **whole ideas
reified as Rust crates that no binary ever linked**:

- the `plugin-exec-*` family (`allow`/`deny`/`counter`/`map`/`trace`/
  `max-calls`/`max-depth`/`logging`) — ADR-0040 products whose limits are
  already inlined in `SandboxHost`;
- `plugin-loader-wasm` / `plugin-loader-web` — faithful implementations of
  ADR-0035, an ADR that is still only `Proposed`;
- `plugin-engine-transform-parallel` — implemented, documented, never reachable
  because its own threshold keeps it on the sequential path;
- `plugin-diag-expression` — superseded by `plugin-ext-diag` + lint-rules-as-data;
- `plugin-source` — a `SourceMap` layer the AST already made redundant.

Deleting these outright would have destroyed the *only* record of the idea.
Keeping them compiled kept the wrong reification alive, cost build time, and
made the workspace lie about what it contains. The old failure mode was that
nobody could tell the difference between "aspirational", "superseded", and
"accidentally unwired" — which is why the ambiguity survived so long.

## Decision

1. **The boundary rule.** Rust holds the **kernel** (the ADR-0040 primitives)
   and **generic tooling** (engines, engines' data-driven machinery, shared
   host contracts). Everything else — policy, behavior, syntax, semantics,
   libraries — is **Handl over a C ABI / SDK-gen contract**. A Rust crate that
   implements policy is a wrongly placed idea, not a contribution. New crates
   must state which of the two Rust roles they fill.

2. **Unwired is a declared state, not an accident.** Every workspace member is
   either depended on by a sibling, or declared in a ledger with a reason.
   Being unreachable is never silently acceptable again.

3. **`project/parked.yaml` is the idea ledger.** Each retired entry carries the
   `idea`, the `adr` that owns it, a `status` (`superseded` / `proposed` /
   `unproven`), a `superseded_by`, and an `unpark` condition naming what would
   bring it back. The code is discarded from `main`; **the idea is kept**.

4. **Retirement protocol.** Classify (aspirational → `proposed`; replaced →
   `superseded`; never wired → `unproven`), add the ledger entry, tag the commit
   before removal as `parked/<family>-<year>` so `git checkout <tag> -- crates/<dir>`
   restores it, then delete the crate, its `members` entry, and its
   `[workspace.dependencies]` line.

5. **The gate is a test, not an audit.** `crates/platform-tests/tests/wiring.rs`
   fails when (a) a retired crate is still a member, (b) a member is an
   undeclared orphan, or (c) a ledger entry is stale (missing ADR, missing
   `unpark`, dangling crate). Orphan detection therefore happens at PR time.

## Consequences

- 14 crates (~2.9k LOC) and `kit-canonical`'s vestigial duplicate `KitManifest`
  (plus `canonical_manifest()` and the `Kit::from_manifest` shim that ignored its
  own argument) were removed; `cargo check --workspace --all-targets` stays green.
- The same pattern was found *inside* a live crate: `host-lsp` kept a whole
  second `.hnl` formatter (`src/formatting.rs`, 318 lines plus `TextEdit`) that
  `textDocument/formatting` no longer used — it was superseded by `plugin-ext-fmt`
  and survived only because ten tests had been written for it. The dead
  formatter, its `TextEdit` shape and those tests are gone; the live
  `format_document_json` bridge (which delegates to the canonical engine) stays.
- Removing the duplicate surfaced a real inconsistency: the LSP's *range*
  formatter had its own line-based indentation and disagreed with `hnl fmt` (it
  indented input the canonical engine leaves untouched). **Fixed in workstream
  A**: `range_formatting.rs` is now a projection of the canonical result — format
  the whole document, emit an edit for the selected lines that changed — and its
  tests assert against `format_source` rather than hand-written strings. The
  remaining hand-rolled indenter is `on_type_formatting.rs`, which cannot be
  delegated by inspection (a formatter rewrites lines that have content; the
  feature exists to indent a *blank* new line) — see `project/TODO.md`.
- The next accidental orphan fails CI rather than waiting for the next sweep.
- Parked ideas stay one `git checkout` away, and the ledger makes their status
  reviewable without reading code.
- **Friction is intentional:** adding a genuinely unwired crate now requires a
  deliberate ledger entry with an ADR and an unpark condition.
- Not addressed here (still open, tracked in `project/TODO.md`): the remaining
  zero-coverage files (mostly the `host-lsp/src/handlers/*` surface, which is
  runtime-reachable but untested), 68 single-use public functions, 179
  `#[ignore]`d tests (172 of them the single unfinished `n-dim` feature), the
  four near-clone `plugin-compile-*/config.rs`+`output.rs` pairs, and two
  implemented-but-never-dispatched lowerers in
  `plugin-compile-js/src/lower/proof.rs` (`Path`/`Equiv`).

## Status

accepted
