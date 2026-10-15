//! # handl-plugin-engine-test
//!
//! Bootstrap test runner (ADR-0089).
//!
//! Discovers, runs, and reports tests written in handl. Discovery merges the
//! test-language fragment (`data/test-language.yaml`) onto the injected base
//! language via `Language::merge` (ADR-0044), so `test`/`property`/
//! `integration_test`/`bench` declarations are first-class syntax; the
//! legacy `test_*` binding convention remains as a fallback. The base
//! runtime — language, module chain, import resolver — is injected through
//! [`runner::TestKitSource`], a `Send + Sync` factory the scheduler invokes
//! per worker thread (ADR-0128 §"The 0.8 seam"); this crate keeps no normal
//! dependency on the canonical kit. Execution dispatches selected tests to
//! the generic `plugin-lib-scheduler` `BoundedScheduler` under a
//! `SchedulePolicy`, selects them via [`Filter`]s — a handl filter arrow
//! from `handl-packages/pred/pred.hnl` evaluated per case against a record
//! projection `{ name, path, tags }` (the `plugin-lib-predicate` crate is
//! deleted; its combinator vocabulary lives in `pred.hnl`),
//! and re-collects results into canonical plan order (parallelism changes
//! timing, never ordering — ADR-0089).
//!
//! This crate is the extracted home of the test runner. Per ADR-0092,
//! `handl-plugin-lib-test` has been deleted: the framework contract is
//! declared in handl (`handl-packages/test/test.hnl`) and the
//! [`framework`] module hosts the temporary Rust handls behind it until
//! the required stdlib primitives (error raise/capture, seeded RNG, clock)
//! land, when the implementations move to pure handl under
//! `handl-packages/test/`.

pub mod config;
pub mod events;
pub mod framework;
pub mod provider;
pub mod report;
pub mod runner;

pub use config::*;
pub use events::*;
pub use provider::*;
pub use report::*;
pub use runner::*;
