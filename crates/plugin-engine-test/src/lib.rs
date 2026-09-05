//! # handl-plugin-engine-test
//!
//! Bootstrap test runner (ADR-0089).
//!
//! Discovers, runs, and reports tests written in handl. The runner parses
//! modules with the canonical kit's Pratt parser, extracts test declarations
//! (bindings whose name starts with `test_`), and evaluates each test body
//! with the tree-walk eval engine. Failures surface as raised errors, which
//! the runner catches and records.
//!
//! This crate is the extracted home of the test runner; the test framework
//! library (`assert`/`property`/`benchmark`/`fuzz`/`snapshot`) remains in
//! `handl-plugin-lib-test` pending ADR-0092.

pub mod runner;

pub use runner::*;
