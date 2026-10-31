+++
id = "ADR-0038"
title = "ADR-0038: Ignored Test Inventory and Triage"
status = "open"
priority = "medium"
+++

---
title: Ignored Test Inventory and Triage
type: adr
date: 2026-08-29
status: accepted
status_note: Accepted
---


# ADR-0038: Ignored Test Inventory and Triage

## Decision


### Keep all 34 ignored tests

None of the ignored tests are removed. Each test is an executable specification for a planned feature. Removing them would lose the contract and force re-derivation of the expected behavior when the feature is implemented.

### Classify by resolution path

Each workstream has a clear unblock condition:

| #   | Workstream                  | Tests | Unblock condition                                                         | Depends on                      |
| --- | --------------------------- | ----- | ------------------------------------------------------------------------- | ------------------------------- |
| 1   | VM eval                     | 11    | Implement `CanonicalVm::eval` tree-walking interpreter                    | —                               |
| 2   | LSP plugins                 | 6     | Wire `ToolingHost` to invoke LSP capabilities with parsed AST             | Parser (done)                   |
| 3   | Format/diagnostics          | 4     | Wire `ToolingHost::format` and `diagnose` to invoke platform capabilities | Formatter (done), Linter (done) |
| 4   | Network/TLS                 | 5     | Add `tls` feature flag or move to CI-only suite                           | —                               |
| 5   | Concurrent stress           | 5     | Fix scheduler race; gate behind `stress` feature flag                     | —                               |
| 6   | HTTP server function values | 2     | Wire `route_function` to VM eval pipeline                                 | Workstream 1                    |
| 7   | Loader/CLI                  | 1     | Expose `load` capability through `ToolingHost`                            | —                               |

### Priority order for un-ignoring

1. **Workstream 3 (format/diagnostics)** — the formatter and linter pipelines are already implemented and registered in `kit-canonical`. The only missing piece is `ToolingHost` calling the `format` and `lint` capabilities. This is the lowest-effort, highest-value unblock.

2. **Workstream 2 (LSP plugins)** — the LSP plugins are implemented and registered. `ToolingHost` needs to parse the source, then invoke `completion`, `goto-def`, and `document-symbols` capabilities with the AST and position. Depends on the parser, which is complete.

3. **Workstream 1 (VM eval)** — the largest workstream. Unblocks workstream 6 as a side effect. Requires implementing the tree-walking interpreter in `plugin-vm-backend-rust-interpreter`.

4. **Workstream 7 (loader/CLI)** — small effort once `ToolingHost` exposes a `load` entry point.

5. **Workstream 4 (network/TLS)** — move behind a `tls` feature flag so they don't run by default. No implementation work needed, just gating.

6. **Workstream 5 (concurrent stress)** — gate behind a `stress` feature flag. Fix the scheduler race separately.

7. **Workstream 6 (HTTP server function values)** — unblocked once workstream 1 is done.

### No feature flags for workstreams 1–3 and 7

Workstreams 1, 2, 3, and 7 are core platform features. Their tests should run by default once un-ignored. No feature flag is needed.

### Feature flags for workstreams 4 and 5

Network/TLS integration tests and concurrent stress tests do not belong in the default `cargo test` run even when passing. They will be gated behind `tls` and `stress` feature flags respectively, and the `#[ignore]` attribute will be replaced by `#[cfg(feature = "...")]`.


## Why

The workspace currently has 34 ignored tests across 10 files in 7 distinct workstreams. These tests were written ahead of their implementing code — they describe expected behavior for features that are stubbed, unwired, or require external infrastructure. The `#[ignore]` attribute keeps them out of the default `cargo test` run while preserving them as executable specifications.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- All 34 ignored tests remain in the codebase as executable specifications.
- The unblock condition for each workstream is explicit and tracked in this ADR.
- Workstreams 4 and 5 will eventually move from `#[ignore]` to `#[cfg(feature)]`.
- Workstreams 1, 2, 3, 6, and 7 will have `#[ignore]` removed when their unblock condition is met.
- This ADR should be updated when a workstream is unblocked, recording the commit that un-ignored the tests.

## Enforcement

- `kit-canonical` — where this is true.
- `plugin-vm-backend-rust-interpreter` — where this is true.
- `plugin-ext-fmt` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0038-ignored-test-inventory-and-triage.md`](../design/0038-ignored-test-inventory-and-triage.md).
