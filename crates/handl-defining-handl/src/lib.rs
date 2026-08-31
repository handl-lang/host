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
//! handl the data-driven engines can evaluate — the constructs migrated to
//! the data-driven model in Phases 4–5 (value, var, the binary arithmetic /
//! comparison ops, unary not, let / stmt.let / stmt.const, if, ternary,
//! function, call, pipe, paren, apply). It is the "handl defining
//! handl": the language spec as data.
//!
//! ## Scope (honest deliverable)
//!
//! Phases 4–5 migrated a **representative subset** of handl's ~50 constructs
//! to the data-driven model. **Full self-hosting of the entire handl language
//! is not achievable yet** — constructs whose eval schemes dispatch to
//! `Foreign` host functions (match, handle, flow, fanout, field-access,
//! import, export, module, template, resolve, json, hello.greet) are not
//! compilable by the bytecode/cranelift engines, and several `Prim` ops
//! (array, object, index, assign) are not yet implemented in any engine. The
//! `type-annot` construct uses a `Bind` with
//! `$payload` (reads `payload["name"]`) but a type annotation has no natural
//! bind name, so it is blocked until the payload convention is clarified.
//! This crate does **not** attempt to describe the full handl language as
//! data. Instead it:
//!
//! 1. Builds the **self-hosting infrastructure**: a [`Language`] value
//!    describing the subset, loadable both from the canonical YAML data files
//!    ([`language`]) and from an authored **JSON data file**
//!    ([`language_from_data_file`]).
//! 2. Ships the **data file** (`data/handl.json`) — the ADR §6 end state:
//!    "authored data files are canonical." The file is the serialized
//!    [`Language`]; loading it requires no Rust code.
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
//! remaining constructs — primarily `Foreign`-dispatched constructs and the
//! unimplemented `Prim` ops), and a standalone-binary bootstrap (writing an
//! executable to disk). Phase 8 proves the *mechanism*, not the
//! standalone-binary artifact.
//!
//! ## Two load paths
//!
//! - [`language`] — assembles the `Language` from the canonical YAML data
//!   files via [`handl_kit_data_canonical::assemble`]. This is the
//!   data-driven path (ADR §6 end state).
//! - [`language_from_data_file`] — loads the `Language` from the authored
//!   `data/handl.json` file (embedded via `include_str!` for portability,
//!   deserialized at runtime). This is the standalone data-file path.
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

/// The self-describing `Language` assembled from the canonical YAML data
/// files.
///
/// This is the canonical data-driven language: the same `Language` produced by
/// [`handl_kit_data_canonical::assemble`] — the full language (tokens, grammar,
/// eval, AST shapes, capabilities) merged into one `Language` value from the
/// 7 YAML files in `handl-kit-data-canonical/data/`. It is "handl defining
/// handl" for the subset.
///
/// This is the data-driven path (ADR §6 end state). The standalone data-file
/// path is [`language_from_data_file`]; both produce an equal `Language`.
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
    /// Whether this case is supported only under the tree-walk engine (not
    /// bytecode/cranelift). When `true`, the compiled-engine tests skip it;
    /// the tree-walk tests still run it. This flags constructs whose `Prim`
    /// op is implemented in the tree-walk engine but not yet in the bytecode
    /// or cranelift engines (an honest engine-coverage gap).
    pub tree_walk_only: bool,
}

/// Build the self-hosting corpus: a set of programs using the data-driven
/// subset constructs that all three eval engines (tree-walk, bytecode,
/// cranelift) can evaluate, plus a small set of tree-walk-only cases.
///
/// Each case's [`Ast`] is constructed manually using the canonical data-driven
/// node kinds and the payload conventions documented by the Phase 4
/// emitters. The Pratt grammar for delimited forms (let/call/if/function) is
/// a placeholder per Phase 4 deviations, so ASTs are hand-built rather than
/// parsed from source.
///
/// The corpus covers the following constructs. Cases marked **(tw-only)** use
/// a `Prim` op implemented in the tree-walk engine but not yet in the
/// bytecode or cranelift engines — they set [`CorpusCase::tree_walk_only`]
/// and are skipped by the compiled-engine tests. Currently no case is
/// tree-walk-only; the flag is retained for future engine-coverage gaps.
///
/// | Case                 | Source model                          | Constructs used                     |
/// |----------------------|---------------------------------------|-------------------------------------|
/// | `literal`            | `42`                                  | value                               |
/// | `arith_add`          | `1 + 2`                               | value, binary.add                   |
/// | `arith_sub`          | `10 - 4`                              | value, binary.sub                   |
/// | `arith_mul`          | `3 * 4`                               | value, binary.mul                   |
/// | `arith_div`          | `20 / 4`                              | value, binary.div                   |
/// | `arith_pow`          | `2 ^ 3`                               | value, binary.pow                   |
/// | `eq_true`            | `3 == 3`                              | value, binary.eq                    |
/// | `eq_false`           | `3 == 4`                              | value, binary.eq                    |
/// | `lt_true`            | `1 < 2`                               | value, binary.lt                    |
/// | `gt_true`            | `2 > 1`                               | value, binary.gt                    |
/// | `gte_true`           | `2 >= 2`                              | value, binary.gte                   |
/// | `gte_false`          | `1 >= 3`                              | value, binary.gte                   |
/// | `lte_true`           | `2 <= 2`                              | value, binary.lte                   |
/// | `lte_false`          | `3 <= 1`                              | value, binary.lte                   |
/// | `neq_true`           | `3 != 4`                              | value, binary.neq                   |
/// | `neq_false`          | `3 != 3`                              | value, binary.neq                   |
/// | `compare_less`       | `1 <=> 2`                             | value, binary.compare               |
/// | `compare_equal`      | `2 <=> 2`                             | value, binary.compare               |
/// | `compare_greater`    | `3 <=> 2`                             | value, binary.compare               |
/// | `typeof_number`      | `typeof 42`                           | value, typeof                       |
/// | `typeof_string`      | `typeof "hi"`                         | value, typeof                       |
/// | `typeof_bool`        | `typeof true`                         | value, typeof                       |
/// | `not_true`           | `!false`                              | value, unary.not                    |
/// | `not_false`          | `!true`                               | value, unary.not                    |
/// | `let_var`            | `let x = 5; x`                        | value, let, var                     |
/// | `stmt_let`           | `let x = 6; x`                        | value, stmt.let, var                |
/// | `stmt_const`         | `const x = 7; x`                      | value, stmt.const, var              |
/// | `if_true`            | `if (true) 1 else 2`                  | value, if                           |
/// | `if_false`           | `if (false) 1 else 2`                 | value, if                           |
/// | `ternary_true`       | `true ? 1 : 2`                        | value, ternary                      |
/// | `ternary_false`      | `false ? 1 : 2`                       | value, ternary                      |
/// | `paren`              | `(42)`                                | value, paren                        |
/// | `apply`              | `apply 42`                            | value, apply                        |
/// | `function_call`      | `let f = (x, y) -> x + y; f(3, 4)`    | function, call, let, var, binary.add|
/// | `pipe_call`          | `let f = (x) -> x + 1; 5 | f`         | function, pipe, let, var, binary.add|
/// | `nested_let`         | `let x = 3; let y = 4; x + y`         | let, var, binary.add                |
/// | `if_with_let`        | `if (true) (let x = 1; x) else 0`     | if, let, var, value                 |
/// | `array_literal`      | `[1, 2, 3]`                           | array                               |
/// | `array_index`        | `[10, 20, 30][1]`                     | index                               |
/// | `object_literal`     | `{"a": 1, "b": 2}`                    | object                              |
/// | `object_index`       | `{"x": 5}["x"]`                       | index                               |
/// | `assign_value`       | `x = 42`                              | assign                              |
///
/// Note: the `call` construct targets **2-argument calls** (the canonical
/// emitter's fixed arity), so the function-call case uses a 2-arg function
/// `f(x, y) -> x + y` applied as `f(3, 4)`. The `pipe` construct's scheme is
/// `App { callee: Child 1, args: [Child 0] }` — it applies the right child
/// (a function) to the left child (a value) — so the pipe case uses a 1-arg
/// function.
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
            tree_walk_only: false,
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
            tree_walk_only: false,
        },
        // 3. Subtraction: 10 - 4
        CorpusCase {
            name: "arith_sub",
            source: "10 - 4",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(10)));
                a.push(AstNode::new("value").with_payload(json!(4)));
                a.push(
                    AstNode::new("binary.sub")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(6),
            tree_walk_only: false,
        },
        // 4. Multiplication: 3 * 4
        CorpusCase {
            name: "arith_mul",
            source: "3 * 4",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(4)));
                a.push(
                    AstNode::new("binary.mul")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(12),
            tree_walk_only: false,
        },
        // 5. Division: 20 / 4
        CorpusCase {
            name: "arith_div",
            source: "20 / 4",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(20)));
                a.push(AstNode::new("value").with_payload(json!(4)));
                a.push(
                    AstNode::new("binary.div")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(5),
            tree_walk_only: false,
        },
        // 6. Power: 2 ^ 3 (`pow` prim now in all engines)
        CorpusCase {
            name: "arith_pow",
            source: "2 ^ 3",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(
                    AstNode::new("binary.pow")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(8.0),
            tree_walk_only: false,
        },
        // 7. Equality true: 3 == 3
        CorpusCase {
            name: "eq_true",
            source: "3 == 3",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(
                    AstNode::new("binary.eq")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 8. Equality false: 3 == 4
        CorpusCase {
            name: "eq_false",
            source: "3 == 4",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(4)));
                a.push(
                    AstNode::new("binary.eq")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(false),
            tree_walk_only: false,
        },
        // 9. Less-than true: 1 < 2
        CorpusCase {
            name: "lt_true",
            source: "1 < 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.lt")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 10. Greater-than true: 2 > 1
        CorpusCase {
            name: "gt_true",
            source: "2 > 1",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(
                    AstNode::new("binary.gt")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 11. Unary not true: !false
        CorpusCase {
            name: "not_true",
            source: "!false",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(false)));
                a.push(
                    AstNode::new("unary.not")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 12. Unary not false: !true
        CorpusCase {
            name: "not_false",
            source: "!true",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(true)));
                a.push(
                    AstNode::new("unary.not")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!(false),
            tree_walk_only: false,
        },
        // 13. Let + var: let x = 5; x
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
            tree_walk_only: false,
        },
        // 14. stmt.let + var: let x = 6; x  (stmt.let uses the same Bind scheme as let)
        CorpusCase {
            name: "stmt_let",
            source: "let x = 6; x",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(6)));
                a.push(AstNode::new("var").with_payload(json!("x")));
                a.push(
                    AstNode::new("stmt.let")
                        .with_payload(json!({"name": "x", "children": [0, 1]})),
                );
                a
            },
            expected: json!(6),
            tree_walk_only: false,
        },
        // 15. stmt.const + var: const x = 7; x  (stmt.const uses the same Bind scheme)
        CorpusCase {
            name: "stmt_const",
            source: "const x = 7; x",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(7)));
                a.push(AstNode::new("var").with_payload(json!("x")));
                a.push(
                    AstNode::new("stmt.const")
                        .with_payload(json!({"name": "x", "children": [0, 1]})),
                );
                a
            },
            expected: json!(7),
            tree_walk_only: false,
        },
        // 16. If true: if (true) 1 else 2
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
            tree_walk_only: false,
        },
        // 17. If false: if (false) 1 else 2
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
            tree_walk_only: false,
        },
        // 18. Ternary true: true ? 1 : 2  (ternary uses the same Branch scheme as if)
        CorpusCase {
            name: "ternary_true",
            source: "true ? 1 : 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(true)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("ternary")
                        .with_payload(json!({"children": [0, 1, 2]})),
                );
                a
            },
            expected: json!(1),
            tree_walk_only: false,
        },
        // 19. Ternary false: false ? 1 : 2
        CorpusCase {
            name: "ternary_false",
            source: "false ? 1 : 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(false)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("ternary")
                        .with_payload(json!({"children": [0, 1, 2]})),
                );
                a
            },
            expected: json!(2),
            tree_walk_only: false,
        },
        // 20. Paren: (42)  (paren is a Child 0 passthrough)
        CorpusCase {
            name: "paren",
            source: "(42)",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(42)));
                a.push(
                    AstNode::new("paren")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!(42),
            tree_walk_only: false,
        },
        // 21. Apply: apply 42  (apply is a Child 0 passthrough)
        CorpusCase {
            name: "apply",
            source: "apply 42",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(42)));
                a.push(
                    AstNode::new("apply")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!(42),
            tree_walk_only: false,
        },
        // 22. Mood (stale — removed in ADR-0026): the `42!` postfix mood
        // marker was replaced by infix keyword syntax (`is`, `must`, `may`,
        // `not`, `when`). The `mood` node kind no longer exists; only
        // `mood.int`, `mood.pot`, `mood.norm`, `mood.neg`, `mood.cnd` remain.
        // This case is intentionally omitted from the corpus.
        // 23. Function + call: let f = (x, y) -> x + y; f(3, 4)
        //     The canonical call emitter targets 2-arg calls, so the function
        //     takes two params.
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
            tree_walk_only: false,
        },
        // 24. Pipe: let f = (x) -> x + 1; 5 | f
        //     pipe scheme is App { callee: Child 1, args: [Child 0] } —
        //     applies the right child (function) to the left child (value).
        CorpusCase {
            name: "pipe_call",
            source: "let f = (x) -> x + 1; 5 | f",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("var").with_payload(json!("x"))); // 0
                a.push(AstNode::new("value").with_payload(json!(1))); // 1
                a.push(
                    AstNode::new("binary.add") // 2: body x + 1
                        .with_payload(json!({"children": [0, 1]})),
                );
                a.push(
                    AstNode::new("function") // 3: (x) -> x + 1
                        .with_payload(json!({"params": ["x"], "body": 2})),
                );
                a.push(AstNode::new("value").with_payload(json!(5))); // 4: left of pipe
                a.push(AstNode::new("var").with_payload(json!("f"))); // 5: right of pipe
                a.push(
                    AstNode::new("pipe") // 6: 5 | f  → App(f, [5])
                        .with_payload(json!({"children": [4, 5]})),
                );
                a.push(
                    AstNode::new("let") // 7: let f = function in pipe
                        .with_payload(json!({"name": "f", "children": [3, 6]})),
                );
                a
            },
            expected: json!(6),
            tree_walk_only: false,
        },
        // 25. Nested let + add: let x = 3; let y = 4; x + y
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
            tree_walk_only: false,
        },
        // 26. If with let in branch: if (true) (let x = 1; x) else 0
        CorpusCase {
            name: "if_with_let",
            source: "if (true) (let x = 1; x) else 0",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(true))); // 0: cond
                a.push(AstNode::new("value").with_payload(json!(1))); // 1
                a.push(AstNode::new("var").with_payload(json!("x"))); // 2
                a.push(
                    AstNode::new("let") // 3: let x = 1 in x
                        .with_payload(json!({"name": "x", "children": [1, 2]})),
                );
                a.push(AstNode::new("value").with_payload(json!(0))); // 4: else
                a.push(
                    AstNode::new("if") // 5
                        .with_payload(json!({"children": [0, 3, 4]})),
                );
                a
            },
            expected: json!(1),
            tree_walk_only: false,
        },
        // 27. Greater-than-or-equal true: 2 >= 2
        CorpusCase {
            name: "gte_true",
            source: "2 >= 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.gte")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 28. Greater-than-or-equal false: 1 >= 3
        CorpusCase {
            name: "gte_false",
            source: "1 >= 3",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(
                    AstNode::new("binary.gte")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(false),
            tree_walk_only: false,
        },
        // 29. Less-than-or-equal true: 2 <= 2
        CorpusCase {
            name: "lte_true",
            source: "2 <= 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.lte")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 30. Less-than-or-equal false: 3 <= 1
        CorpusCase {
            name: "lte_false",
            source: "3 <= 1",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(
                    AstNode::new("binary.lte")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(false),
            tree_walk_only: false,
        },
        // 31. Not-equal true: 3 != 4
        CorpusCase {
            name: "neq_true",
            source: "3 != 4",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(4)));
                a.push(
                    AstNode::new("binary.neq")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(true),
            tree_walk_only: false,
        },
        // 32. Not-equal false: 3 != 3
        CorpusCase {
            name: "neq_false",
            source: "3 != 3",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(
                    AstNode::new("binary.neq")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(false),
            tree_walk_only: false,
        },
        // 33. Compare less: 1 <=> 2 → -1
        CorpusCase {
            name: "compare_less",
            source: "1 <=> 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(1)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.compare")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(-1),
            tree_walk_only: false,
        },
        // 34. Compare equal: 2 <=> 2 → 0
        CorpusCase {
            name: "compare_equal",
            source: "2 <=> 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.compare")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(0),
            tree_walk_only: false,
        },
        // 35. Compare greater: 3 <=> 2 → 1
        CorpusCase {
            name: "compare_greater",
            source: "3 <=> 2",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(3)));
                a.push(AstNode::new("value").with_payload(json!(2)));
                a.push(
                    AstNode::new("binary.compare")
                        .with_payload(json!({"children": [0, 1]})),
                );
                a
            },
            expected: json!(1),
            tree_walk_only: false,
        },
        // 36. typeof number: typeof 42 → "number"
        CorpusCase {
            name: "typeof_number",
            source: "typeof 42",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(42)));
                a.push(
                    AstNode::new("typeof")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!("number"),
            tree_walk_only: false,
        },
        // 37. typeof string: typeof "hi" → "string"
        CorpusCase {
            name: "typeof_string",
            source: "typeof \"hi\"",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!("hi")));
                a.push(
                    AstNode::new("typeof")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!("string"),
            tree_walk_only: false,
        },
        // 38. typeof bool: typeof true → "bool"
        CorpusCase {
            name: "typeof_bool",
            source: "typeof true",
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("value").with_payload(json!(true)));
                a.push(
                    AstNode::new("typeof")
                        .with_payload(json!({"children": [0]})),
                );
                a
            },
            expected: json!("bool"),
            tree_walk_only: false,
        },
        // 39. Array literal: [1, 2, 3] → [1, 2, 3]
        //     The `array` prim reads `elements` directly from the payload.
        CorpusCase {
            name: "array_literal",
            source: "[1, 2, 3]",
            ast: {
                let mut a = Ast::new();
                a.push(
                    AstNode::new("array")
                        .with_payload(json!({"elements": [1, 2, 3]})),
                );
                a
            },
            expected: json!([1, 2, 3]),
            tree_walk_only: false,
        },
        // 40. Array index: [10, 20, 30][1] → 20
        //     The `index` prim reads `target` and `index` from the payload.
        CorpusCase {
            name: "array_index",
            source: "[10, 20, 30][1]",
            ast: {
                let mut a = Ast::new();
                a.push(
                    AstNode::new("index")
                        .with_payload(json!({"target": [10, 20, 30], "index": 1})),
                );
                a
            },
            expected: json!(20),
            tree_walk_only: false,
        },
        // 41. Object literal: {"a": 1, "b": 2} → {"a": 1, "b": 2}
        //     The `object` prim reads `entries` (array of {key, value}) from
        //     the payload and builds a JSON object.
        CorpusCase {
            name: "object_literal",
            source: "{\"a\": 1, \"b\": 2}",
            ast: {
                let mut a = Ast::new();
                a.push(
                    AstNode::new("object").with_payload(json!({
                        "entries": [
                            {"key": "a", "value": 1},
                            {"key": "b", "value": 2}
                        ]
                    })),
                );
                a
            },
            expected: json!({"a": 1, "b": 2}),
            tree_walk_only: false,
        },
        // 42. Object index: {"x": 5}["x"] → 5
        //     The `index` prim with an object target and string key.
        CorpusCase {
            name: "object_index",
            source: "{\"x\": 5}[\"x\"]",
            ast: {
                let mut a = Ast::new();
                a.push(
                    AstNode::new("index")
                        .with_payload(json!({"target": {"x": 5}, "index": "x"})),
                );
                a
            },
            expected: json!(5),
            tree_walk_only: false,
        },
        // 43. Assign: x = 42 → 42
        //     The `assign` prim reads `value` from the payload and returns it.
        CorpusCase {
            name: "assign_value",
            source: "x = 42",
            ast: {
                let mut a = Ast::new();
                a.push(
                    AstNode::new("assign")
                        .with_payload(json!({"target": "x", "value": 42})),
                );
                a
            },
            expected: json!(42),
            tree_walk_only: false,
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
        assert!(c.len() >= 38, "corpus should have at least 38 cases");
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
