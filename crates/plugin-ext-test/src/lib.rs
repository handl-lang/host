//! `handl-plugin-ext-test` — the `test` capability adapter.
//!
//! Exposes the `hnl test` runner (ADR-0089) through the `plugin-ext-*`
//! capability seam, so a host can dispatch discovery + execution uniformly
//! with `fmt`, `lint`, and `typecheck`.
//!
//! The implementation is [`plugin::TestPlugin`], a thin adapter over
//! [`hnl_engine_test`] — the *same* functions `host-cli` calls, so
//! the capability path and the direct call cannot drift.
//!
//! # Division of responsibility
//!
//! The capability is **policy-free**: it does not decide default paths (the
//! caller passes the resolved list), does not choose the reporter's output
//! stream, and does not set a process exit code. It returns structured
//! results — including the *rendered* report, because the engine's report
//! types are not `Serialize` — and the caller decides presentation.
//!
//! The one exception is `cache`: the ADR-0089 on-disk result cache belongs to
//! the runner, so `cache: true` loads and saves it here, at the runner's
//! existing path (`.handl/cache/test-runner/cache.json` under the CWD).
//!
//! # No `language` payload
//!
//! As with `typecheck` (see `plugin-ext-typecheck`), the runner obtains its
//! language from the single canonical assembly path, so there is no
//! ADR-0124 divergence hazard between the CLI and this plugin — unlike
//! `lint`, where the caller and the plugin assembled different "canonical"
//! languages.

pub mod plugin;

pub use plugin::{TEST_CAPABILITY, TestPlugin};
