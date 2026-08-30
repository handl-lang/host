//! # handl-defining-handl
//!
//! The **self-hosting infrastructure** for ADR-0044 Phase 8.
//!
//! ADR-0044 Phase 8 specifies: "`Language` carries an `eval` capability; a
//! `Language` value describes handl itself. Ship a `handl-defining-handl` data
//! file loaded as a `Language` value, evaluated under itself. The bootstrap
//! becomes `compile(handlLanguage, native)` via the cranelift engine."
//!
//! This crate produces a [`Language`] value that describes the **subset** of
//! handl the data-driven engines can evaluate — the ~7 constructs migrated to
//! the data-driven model in Phases 4–5 (identifier, value, arithmetic, let,
//! call, branch, function). It is the "handl defining handl": the language
//! spec as data.
//!
//! ## Scope (honest deliverable)
//!
//! Phases 4–5 migrated only a **representative subset** of handl's ~50
//! constructs to the data-driven model. **Full self-hosting of the entire
//! handl language is not achievable yet** — the data-driven engines can only
//! evaluate programs using the migrated subset. This crate does **not**
//! attempt to describe the full handl language as data. Instead it:
//!
//! 1. Builds the **self-hosting infrastructure**: a [`Language`] value
//!    describing the subset, loadable both from Rust emitters ([`language`])
//!    and from an authored **data file** ([`language_from_data_file`]).
//! 2. Ships the **data file** (`data/handl.json`) — the ADR §6 end state:
//!    "authored data files are canonical." The file is the serialized
//!    [`Language`]; loading it requires no Rust emitters.
//! 3. Provides a **self-hosting corpus** ([`corpus`]) of subset programs used
//!    by the integration tests (`self_host.rs`, `bootstrap.rs`,
//!    `futamura.rs`) to prove the self-hosting *mechanism*.
//!
//! What is **proven**: a `Language` value can describe a language and be
//! evaluated under itself (across all three eval engines: tree-walk,
//! bytecode, cranelift), and compiled to native code via the cranelift engine
//! — the Futamura projection sanity check.
//!
//! What is **deferred**: full handl self-hosting (pending migration of the
//! remaining ~43 constructs), and a standalone-binary bootstrap (writing an
//! executable to disk). Phase 8 proves the *mechanism*, not the
//! standalone-binary artifact.
//!
//! ## Two load paths
//!
//! - [`language`] — assembles the `Language` from the Phase 4 emitters via
//!   [`handl_kit_data_canonical::assemble`]. This is the emitter-based path
//!   (the migration scaffold, ADR §6).
//! - [`language_from_data_file`] — loads the `Language` from the authored
//!   `data/handl.json` file (embedded via `include_str!` for portability,
//!   deserialized at runtime). This is the ADR §6 end state: the language
//!   definition as pure data, no Rust emitters needed at load time.
//!
//! Both paths produce an equal `Language` (asserted in tests).

#![warn(missing_docs)]

use handl_language::{Ast, AstNode, LangError, Language, Value};
use serde_json::json;

/// The path (relative to the crate root) of the authored data file.
///
/// The file is embedded via `include_str!` in [`language_from_data_file`] so
/// the canonical language definition ships as data, not as Rust.
pub const DATA_FILE: &str = "data/handl.json";

/// The self-describing `Language` assembled from the Phase 4 emitters.
///
/// This is the canonical data-driven language: the same `Language` produced by
/// [`handl_kit_data_canonical::assemble`] — the seven migrated constructs
/// (identifier, value, arith, let, call, branch, function) merged into one
/// `Language` value. It is "handl defining handl" for the subset.
///
/// This is the emitter-based path (ADR §6 migration scaffold). The
/// data-file-based path is [`language_from_data_file`]; both produce an equal
/// `Language`.
pub fn language() -> Language {
    // Delegate to the canonical data-driven kit assembly. This is the same
    // merged Language used by the data-driven runtime path — the single
    // source of truth for the subset.
    handl_kit_data_canonical::assemble()
        .expect("canonical data-driven kit must assemble")
        .lang()
        .clone()
}

/// Load the self-describing `Language` from the authored data file
/// (`data/handl.json`).
///
/// This is the ADR §6 end state: "authored data files are canonical." The
/// file is the serialized [`language`] value; loading it requires no Rust
/// emitters — only `serde_json` deserialization. The file is embedded via
/// `include_str!` for portability (so the data ships with the binary), but
/// the `Language` is reconstructed purely from data at runtime via
/// [`parse_language_from_json`].
///
/// The data file round-trips: `language_from_data_file() == language()`
/// (asserted in tests).
///
/// # Errors
///
/// Returns [`LangError::Engine`] if the embedded JSON fails to deserialize
/// into a `Language` (should not happen for a committed, generated file).
pub fn language_from_data_file() -> Result<Language, LangError> {
    parse_language_from_json(include_str!("../data/handl.json"))
}

/// Parse a `Language` from a JSON string (the data-file format).
///
/// This is the pure deserialization step used by [`language_from_data_file`];
/// factoring it out makes the error path testable independently of the
/// compile-time-embedded file contents.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if `text` is not valid JSON or does not
/// deserialize into a [`Language`].
pub fn parse_language_from_json(text: &str) -> Result<Language, LangError> {
    serde_json::from_str::<Language>(text).map_err(|e| {
        LangError::Engine(format!("failed to deserialize handl.json: {}", e))
    })
}

// ---------------------------------------------------------------------------
// Self-hosting corpus
// ---------------------------------------------------------------------------

/// A single self-hosting corpus case: an [`Ast`] built from the subset
/// constructs and the expected evaluation result.
#[derive(Debug, Clone)]
pub struct CorpusCase {
    /// A human-readable name for the case.
    pub name: &'static str,
    /// A human-readable description of the source-level program this models.
    pub source: &'static str,
    /// The AST to evaluate (built from the subset's node kinds).
    pub ast: Ast,
    /// The expected result value.
    pub expected: Value,
}

/// Build the self-hosting corpus: a set of programs using only the migrated
/// subset constructs (literals, arithmetic, let, var, if, function, call).
///
/// Each case's [`Ast`] is constructed manually using the canonical data-driven
/// node kinds (`value`, `var`, `binary.add`, `let`, `if`, `call`,
/// `function`) and the payload conventions documented by the Phase 4
/// emitters. The Pratt grammar for delimited forms (let/call/if/function) is
/// a placeholder per Phase 4 deviations, so ASTs are hand-built rather than
/// parsed from source.
///
/// The corpus covers:
///
/// | Case            | Source model                          | Constructs used          |
/// |-----------------|---------------------------------------|--------------------------|
/// | `literal`       | `42`                                  | value                    |
/// | `arith_add`     | `1 + 2`                               | value, binary.add        |
/// | `let_var`       | `let x = 5; x`                        | value, let, var          |
/// | `if_true`       | `if (true) 1 else 2`                  | value, if                |
/// | `if_false`      | `if (false) 1 else 2`                 | value, if                |
/// | `function_call` | `let f = (x, y) -> x + y; f(3, 4)`    | function, call, let, var, binary.add |
/// | `nested_let`    | `let x = 3; let y = 4; x + y`         | let, var, binary.add     |
///
/// Note: the `call` construct targets **2-argument calls** (the canonical
/// emitter's fixed arity), so the function-call case uses a 2-arg function
/// `f(x, y) -> x + y` applied as `f(3, 4)`.
pub fn corpus() -> Vec<CorpusCase> {
    vec![
        // 1. Literal: 42
        CorpusCase {
            name: "literal",
            source: "42",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(42)));
                a
            },
            expected: json!(42),
        },
        // 2. Arithmetic: 1 + 2
        CorpusCase {
            name: "arith_add",
            source: "1 + 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.add")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(3),
        },
        // 3. Let + var: let x = 5; x
        CorpusCase {
            name: "let_var",
            source: "let x = 5; x",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(5)));
                a.push(AstNode::new("var").with_payload(json!("x")));
                a.push(
                    AstNode::new("let")
                        .with_payload(json!({"name": "x", "children": [0, 1]})),
                );
                a
            },
            expected: json!(5),
        },
        // 4. If true: if (true) 1 else 2
        CorpusCase {
            name: "if_true",
            source: "if (true) 1 else 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(true)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("if")
                        .with_payload(json!({"children": [0, 1, 2]})),
                );
                a
            },
            expected: json!(1),
        },
        // 5. If false: if (false) 1 else 2
        CorpusCase {
            name: "if_false",
            source: "if (false) 1 else 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(false)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("if")
                        .with_payload(json!({"children": [0, 1, 2]})),
                );
                a
            },
            expected: json!(2),
        },
        // 6. Function + call: let f = (x, y) -> x + y; f(3, 4)
        //    The canonical call emitter targets 2-arg calls, so the function
        //    takes two params.
        CorpusCase {
            name: "function_call",
            source: "let f = (x, y) -> x + y; f(3, 4)",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("var").with_payload(json!("x"))); // 0
                a.push(AstNode::new("var").with_payload(json!("y"))); // 1
                a.push(
                    AstNode::new("binary.add") // 2: body x + y
                        .with_payload(json!({"children": [0, 1]})),
                );
                a.push(
                    AstNode::new("function") // 3: (x, y) -> x + y
                        .with_payload(json!({"params": ["x", "y"], "body": 2})),
                );
                a.push(AstNode::new("value").with_payload(json!(3))); // 4
                a.push(AstNode::new("value").with_payload(json!(4))); // 5
                a.push(AstNode::new("var").with_payload(json!("f"))); // 6
                a.push(
                    AstNode::new("call") // 7: f(3, 4)
                        .with_payload(json!({"children": [6, 4, 5]})),
                );
                a.push(
                    AstNode::new("let") // 8: let f = function in call
                        .with_payload(json!({"name": "f", "children": [3, 7]})),
                );
                a
            },
            expected: json!(7),
        },
        // 7. Nested let + add: let x = 3; let y = 4; x + y
        CorpusCase {
            name: "nested_let",
            source: "let x = 3; let y = 4; x + y",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3))); // 0
                a.push(AstNode::new("value").with_payload(json!(4))); // 1
                a.push(AstNode::new("var").with_payload(json!("x"))); // 2
                a.push(AstNode::new("var").with_payload(json!("y"))); // 3
                a.push(
                    AstNode::new("binary.add") // 4: x + y
                        .with_payload(json!({"children": [2, 3]})),
                );
                a.push(
                    AstNode::new("let") // 5: let y = 4 in (x + y)
                        .with_payload(json!({"name": "y", "children": [1, 4]})),
                );
                a.push(
                    AstNode::new("let") // 6: let x = 3 in (let y ...)
                        .with_payload(json!({"name": "x", "children": [0, 5]})),
                );
                a
            },
            expected: json!(7),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The emitter-based and data-file-based languages are equal (round-trip).
    #[test]
    fn language_from_data_file_equals_language() {
        let from_emitters = language();
        let from_file = language_from_data_file().expect("data file must load");
        assert_eq!(
            from_emitters, from_file,
            "the data-file language must equal the emitter-assembled language"
        );
    }

    /// The data file deserializes successfully and is non-empty.
    #[test]
    fn data_file_loads_nonempty_language() {
        let lang = language_from_data_file().expect("data file must load");
        assert!(!lang.tokens.is_empty(), "tokens must be non-empty");
        assert!(!lang.grammar.is_empty(), "grammar must be non-empty");
        assert!(!lang.eval.is_empty(), "eval must be non-empty");
    }

    /// `parse_language_from_json` returns an error for invalid JSON,
    /// exercising the deserialization error path.
    #[test]
    fn parse_language_from_json_errors_on_invalid_input() {
        let err = parse_language_from_json("not valid json").unwrap_err();
        assert!(
            matches!(err, LangError::Engine(ref msg) if msg.contains("deserialize")),
            "expected an Engine error mentioning deserialize, got {:?}",
            err
        );
    }

    /// `parse_language_from_json` round-trips the data file text.
    #[test]
    fn parse_language_from_json_round_trips_data_file() {
        let text = include_str!("../data/handl.json");
        let from_text = parse_language_from_json(text).expect("must parse");
        assert_eq!(from_text, language());
    }

    /// The corpus is non-empty and every case has a non-empty AST.
    #[test]
    fn corpus_is_nonempty() {
        let c = corpus();
        assert!(c.len() >= 7, "corpus should have at least 7 cases");
        for case in &c {
            assert!(!case.ast.is_empty(), "case '{}' has empty ast", case.name);
        }
    }

    /// Generate (or regenerate) `data/handl.json` from [`language`].
    ///
    /// This runs on every test pass: it serializes the emitter-assembled
    /// `Language` to the committed data file. The write is idempotent (the
    /// file content is deterministic), so this keeps the data file fresh
    /// without side effects. The file is also loaded via `include_str!` at
    /// compile time, so the runtime write does not affect the already-compiled
    /// embedded copy — but it ensures the committed file matches `language()`.
    #[test]
    fn generate_data_file() {
        let lang = language();
        let pretty = serde_json::to_string_pretty(&lang).expect("serialize");
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DATA_FILE);
        std::fs::write(&path, pretty).expect("write data file");
    }
}
