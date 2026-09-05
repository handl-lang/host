# ADR-0038: Ignored Test Inventory and Triage

## Status

Accepted

## Context

The workspace currently has 34 ignored tests across 10 files in 7 distinct workstreams. These tests were written ahead of their implementing code — they describe expected behavior for features that are stubbed, unwired, or require external infrastructure. The `#[ignore]` attribute keeps them out of the default `cargo test` run while preserving them as executable specifications.

This ADR inventories every ignored test, classifies why it is ignored, and records the decision for each: keep as-is, remove, or un-ignore as part of a named workstream. This prevents ignored tests from becoming invisible dead code and makes the path to green explicit.

## Inventory

### 1. VM evaluation stubbed (11 tests)

**Files:**

- `crates/plugin-vm-common/src/middleware_vm.rs` (9 tests)
- `crates/plugin-vm-common/src/host/host_tests.rs` (2 tests)

**Reason:** `CanonicalVm::eval` and `CanonicalVm::eval_async` in `plugin-vm-backend-rust-interpreter` return `Error::PrimitiveError("not implemented")`. The tests exercise middleware tracing, async evaluation, and host-bound eval callbacks — all of which require a working tree-walking interpreter.

**Tests:**

- `middleware_records_sync_trace`
- `middleware_records_async_trace`
- `middleware_trace_summarizes_nested_calls`
- `middleware_trace_distinguishes_sync_and_async`
- `middleware_trace_records_handler_entry`
- `middleware_trace_records_handler_match`
- `middleware_trace_records_handler_fallthrough`
- `middleware_trace_records_yield_and_resume`
- `middleware_trace_summarizes_let_binding`
- `host_eval_returns_value`
- `host_eval_propagates_error`

### 2. LSP plugins stubbed (6 tests)

**File:** `crates/tooling/tests/lsp_plugins.rs`

**Reason:** `ToolingHost` wires `BasicCompletion`, `BasicGotoDef`, and `BasicDocumentSymbols` as plugins, but the tooling layer does not yet route parsed AST or token spans into the LSP capability handlers. The LSP plugins exist but their `run` methods return `NotImplemented`.

**Tests:**

- `complete_includes_let_binding`
- `complete_offers_keyword`
- `goto_def_finds_let_binding`
- `document_symbols_for_let_and_const`
- `incremental_change_appends_text`
- `format_still_works`

### 3. Format and diagnostics not implemented (4 tests)

**File:** `crates/tooling/tests/perf.rs`

**Reason:** `ToolingHost::format` and `ToolingHost::diagnose` are stubbed. The formatter pipeline (`BasicFormatter` + `FormatBehavior` rules) exists in `plugin-ext-fmt` and `kit-canonical` registers it, but `ToolingHost` does not yet invoke the `format` capability. Similarly, the diagnostics/lint pipeline (`BasicLinter` + `LintBehavior`) is registered but not called by the tooling layer.

**Tests:**

- `format_long_pipe_100`
- `format_long_pipe_1000`
- `format_long_pipe_10000` (also notes a debug-build stack overflow for 10k nested pipes)
- `lsp_diagnostics_long_500`

### 4. Network/TLS integration (5 tests)

**Files:**

- `crates/plugin-lib-ws-rfc-native/src/module.rs` (3 tests)
- `crates/plugin-lib-tls-native/src/tests.rs` (2 tests)

**Reason:** These tests require either a live TLS handshake against a remote server or a configured `rustls` crypto provider. The workspace does not bundle a default crypto provider, and CI does not provision TLS certificates. These are integration tests that belong behind a feature flag or a CI-only runner, not in the default test suite.

**Tests:**

- WebSocket TLS connect, send, and close (3 tests in `module.rs`)
- TLS connector tests (2 tests in `tests.rs`)

### 5. Concurrent stress tests (5 tests)

**Files:**

- `crates/plugin-lib-concurrent/tests/stress.rs` (4 tests)
- `crates/plugin-lib-concurrent/src/scheduler.rs` (1 test)

**Reason:** High-contention stress tests that spawn many threads and assert scheduling fairness. They are flaky under load and one has a known race condition in the thread pool scheduler. These are not unit tests — they are performance/fairness regressions that require controlled environments.

**Tests:**

- 4 stress tests in `stress.rs` (no explicit reason string)
- 1 scheduler test (`flaky race; see thread pool scheduler`)

### 6. HTTP server function-value handlers not wired (2 tests)

**File:** `crates/plugin-lib-http-server-std/tests/integration_tests.rs`

**Reason:** `StdHttpServer::route_function` accepts a `FunctionValue` (with params, body, and patterns) but the server does not yet evaluate the function body through the VM. The handler stores the function but returns `NotImplemented` on request. Wiring requires the VM eval pipeline (workstream 1) to be functional.

**Tests:**

- `function_value_handler_returns_string`
- `function_value_handler_uses_path_param`

### 7. Loader/CLI wiring not implemented (1 test)

**File:** `crates/tooling/tests/loader.rs`

**Reason:** The test loads a native cdylib plugin through `NativeLoader` and dispatches an `echo` capability. The `NativeLoader` plugin exists and the test logic is correct, but `ToolingHost` does not yet expose a `load` capability entry point that the CLI can call. The test is a full integration test that requires a compiled cdylib in `target/debug`.

**Test:**

- `tooling_host_loads_native_plugin_and_dispatches_command`

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

## Consequences

- All 34 ignored tests remain in the codebase as executable specifications.
- The unblock condition for each workstream is explicit and tracked in this ADR.
- Workstreams 4 and 5 will eventually move from `#[ignore]` to `#[cfg(feature)]`.
- Workstreams 1, 2, 3, 6, and 7 will have `#[ignore]` removed when their unblock condition is met.
- This ADR should be updated when a workstream is unblocked, recording the commit that un-ignored the tests.

## References

- ADR-0024: Parser Common (Pratt parser architecture — now migrated to `ParserBehavior`)
- ADR-0027: VM Crate Restructure (CanonicalVm structure)
- ADR-0029: CLI as Capability Orchestrator (loader/CLI wiring)
- ADR-0031: Platform Abstractions (formatter, linter, LSP plugin model)
- ADR-0036: Host Applier as Plugin (ToolingHost context)
- ADR-0037: Plugin Resolution and Excludes
