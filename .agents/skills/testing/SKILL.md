---
name: testing
description: Testing conventions for the Handl platform. Use when writing tests, setting up test files, or mocking dependencies.
---

# Testing

Two test surfaces: Rust (`cargo`) and Handl (`.test.hnl`).

## Rust tests

- `cargo test --workspace`, `cargo test -p <crate-name>`, `cargo test -p <crate> --test <name>`.
- **Unit tests**: colocated `*_test.rs` modules wired via `#[cfg(test)] mod foo_test;`
  in the parent file (see `crates/plugin-sdkgen-backend-c/src/collect.rs` +
  `collect_test.rs`).
- **Integration/e2e tests**: `crates/<crate>/tests/*.rs` (e.g.
  `plugin-sdkgen-backend-c/tests/c_backend.rs`, `plugin-lib-stdlib/tests/math.rs`).
- Tests live in `tests/`, not inline `#[cfg(test)]` in `src/ops/*` — check the
  target crate's existing convention first.
- Live FFI tests (real socketpair/ioctl) exist in `plugin-ffi/src/tests.rs` —
  keep them deterministic and self-contained.

## Handl tests

- `*.test.hnl` files next to the package they exercise
  (`handl-packages/bytes/bytes.test.hnl`, `handl-packages/utils/math.test.hnl`).
- Convention: `from "<pkg>" import * as <alias>` then `test_<name> := assert...`
  cases; run with `hnl test <path>` (discovery in `plugin-engine-test`).
- The test framework is pure handl (`handl-packages/test/`) — `assert.*` is a
  Rust side-channel for failure reporting only.

## Mocking / stubbing

- No test-mock framework: use the **contract + backend** pattern — a contract
  package (`rand/`, `os/`, `uuid/`) plus swappable backends (`rand-c`,
  `rand-stub`, `os-c`, `os-stub`). Tests bind the stub backend.

## Gate

`scripts/dogfood-gate.sh` = `hnl fmt --check && hnl lint && hnl typecheck &&
hnl test` over `handl-packages/` + `examples/` — a **blocking CI job**. Keep it
green.
