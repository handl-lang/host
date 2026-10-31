+++
id = "ADR-0092"
title = "ADR-0092: Test Framework in Handl"
status = "open"
priority = "medium"
+++

---
title: Test Framework in Handl
type: adr
date: 2026-09-05
status: implemented
status_note: Implemented — the framework is handl; `plugin-lib-test` is deleted.
---

# ADR-0092: Test Framework in Handl

## Decision

1. **Target layout**: the framework lives in `handl-packages/test/` as handl
   modules — `assert`, `property`, `benchmark`, `fuzz`, `snapshot` — with the
   `op`/`handl` declarations in `test.hnl`.
2. **Migration per op**: each framework operation moves from Rust to a handl
   module, one op at a time, with parity tests.
3. **Crate boundary**: `plugin-lib-test` is deleted; the runner
   (`engine-test`) consumes the framework through the module chain.
4. **Relationship to ADR-0089**: the runner stays Rust; this ADR moves only the
   framework contract.
5. **Relationship to ADR-0077**: the framework is part of the stdlib migration;
   every public arrow carries a doc comment and an axiom.
6. **What stays Rust**: the test runner only.

## Why

The framework library was Rust `Function` bodies beside a Rust runner, so a
framework change meant a rebuild and the assertion semantics could not be
inspected. Handl modules make the framework data and leave only the runner in
Rust.

## Not doing
- **Keep the framework in Rust** — the duplication ADR-0077 removes.
- **Move the runner too** — ADR-0089 keeps it Rust; RFC-0005 tracks the port.
- **One big-bang migration** — one op at a time with parity tests.

## Consequences
- The framework is handl; `plugin-lib-test` is gone.
- The runner consumes the framework through the module chain.
- Each op has a parity test before the Rust body is deleted.

## Enforcement
- `handl-packages/test/` — `test.hnl` and the five modules.
- `engine-test/` — the runner and its framework dependency.
- Parity tests per migrated op.

## Design
+Target layout, per-op migration, crate boundary and the ADR relationships:
+[`design/0092-test-framework-in-handl.md`](../design/0092-test-framework-in-handl.md).
