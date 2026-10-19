//! Shared fixtures for the handl benchmark suite.
//!
//! This crate restores the pre-`bcf31afa` `crates/bench` criterion suite
//! (deleted in the "obsolete crates and files" sweep) against the
//! post-refactor engine APIs. The original benches were written for crates
//! that no longer exist (`handl-lexer`, `handl-parser-canonical`,
//! `handl-typecheck-infer`, `handl-compiler`, …); see `README.md` for which
//! of the six original benches are restored and which still need an API
//! mapping decision.

/// A representative canonical statement: member access, a call, and literal
/// arguments (number, negative float, string, array, record).
///
/// Deliberately a *single* statement — the Pratt parser parses one statement
/// per call, so a multi-statement module would need the module splitter and
/// would measure that rather than the grammar.
///
/// The pre-refactor sample (`handle foo as bar(...)`) was retired-dialect
/// syntax and no longer parses.
pub const SAMPLE: &str =
    r#"qux.handle.nested.call(123.456, -2.5, "hello", [1, 2, 3], { a: true, b: null })"#;

/// A self-contained, **evaluable** module for the pipeline bench.
///
/// [`SAMPLE`] deliberately leaves its callee unbound so it exercises parsing
/// without needing a runtime; evaluating it fails with
/// `function 'qux…' not found`. Evaluation needs every name bound, so this
/// binds the function first.
///
/// Note: this previously used the `|>` pipe operator (`"f = x ~> x; 1 |> f"`),
/// inherited from `kit-canonical/benches/perf.rs`. `|>` was **removed** from
/// the canonical grammar (`handl-kit-data-prelude/data/ast.yaml`,
/// "Removed: … pipe …"), so the sample no longer parsed and
/// `pipeline/parse_and_eval` panicked on its first iteration. Plain
/// application is the supported form.
pub const EVAL_SAMPLE: &str = "f = x ~> x; f 1";

/// An inference-heavy module for the `infer` bench.
///
/// Three chained arrows plus an application, so the `.hnl` model has to
/// synthesize a type for each binding *and* for the application site — the
/// shape `handl-typecheck-infer` used to be measured on. Validated against
/// `hnl typecheck` (`<path>: ok`).
pub const INFERENCE_SAMPLE: &str = "f = x ~> x; g = y ~> f y; h = z ~> g z; h 1";
