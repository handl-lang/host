//! Shared fixtures for the handl benchmark suite.
//!
//! This crate restores the pre-`bcf31afa` `crates/bench` criterion suite
//! (deleted in the "obsolete crates and files" sweep) against the
//! post-refactor engine APIs. The original benches were written for crates
//! that no longer exist (`handl-lexer`, `handl-parser-canonical`,
//! `handl-typecheck-infer`, `handl-compiler`, …); see `README.md` for which
//! of the six original benches are restored and which still need an API
//! mapping decision.

use handl_language::Language;

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

/// The canonical language, assembled from the embedded YAML fragments.
pub fn canonical_lang() -> Language {
    handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles")
}
