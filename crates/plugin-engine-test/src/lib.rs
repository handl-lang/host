//! # handl-plugin-engine-test
//!
//! Bootstrap test runner (ADR-0089).
//!
//! Discovers, runs, and reports tests written in handl. Discovery merges the
//! test-language fragment (`data/test-language.yaml`) onto the canonical kit
//! via `Language::merge` (ADR-0044), so `test`/`property`/`integration_test`/
//! `bench` declarations are first-class syntax; the legacy `test_*` binding
//! convention remains as a fallback. Execution dispatches selected tests to
//! the generic `plugin-lib-scheduler` `BoundedScheduler` under a
//! `SchedulePolicy`, selects them via `plugin-lib-predicate` `Predicate`s,
//! and re-collects results into canonical plan order (parallelism changes
//! timing, never ordering — ADR-0089).
//!
//! This crate is the extracted home of the test runner; the test framework
//! library (`assert`/`property`/`benchmark`/`fuzz`/`snapshot`) remains in
//! `handl-plugin-lib-test` pending ADR-0092.

pub mod config;
pub mod events;
pub mod provider;
pub mod report;
pub mod runner;

pub use config::*;
pub use events::*;
pub use provider::*;
pub use report::*;
pub use runner::*;
