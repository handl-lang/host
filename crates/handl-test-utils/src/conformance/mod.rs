//! # Conformance test harness
//!
//! Shared conformance test infrastructure for `hnl-engine-*` engines
//! (ADR-0044 testing strategy). Contains:
//!
//! - [`corpus`] — `(Language, Ast, expected Value)` and
//!   `(Language, Tokens, expected Ast)` triples as data.
//! - [`run`] — harness functions that run the corpus against any eval or
//!   parse engine.
//!
//! Every eval engine crate runs the full eval conformance suite; every parse
//! engine crate runs the full parse conformance suite. A divergence is a test
//! failure.

pub mod corpus;
pub mod run;

pub use corpus::{EvalCase, ParseCase, eval_corpus, parse_corpus};
pub use run::{run_eval_conformance, run_parse_conformance};
