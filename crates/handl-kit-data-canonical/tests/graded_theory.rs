//! Tests for the graded-theory declaration surface syntax (ADR-0098/0100).
//!
//! The canonical kit recognizes six declaration forms — `sort`, `grade`,
//! `op`, `handler`, `rewrite`, and `law` — plus the `=>` 2-cell arrow
//! (`token.fat_arrow`) and the `grades` clause keyword. These tests verify
//! the lexer and Pratt parser produce the declared node kinds and child
//! roles.

use handl_kit_data_canonical;
use handl_language::{Ast, AstNode, Language, Source};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
use std::rc::Rc;

/// Assemble the full canonical kit and return its language.
fn full_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    kit.lang().clone()
}

/// Build a runtime with the canonical language + Pratt parse + tree-walk eval.
fn full_runtime() -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    Runtime::from_registry(kit.lang().clone(), &registry, &config).expect("runtime binds engines")
}

/// Lex + parse `src` and return the AST (root = last node).
fn parse(src: &str) -> Ast {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex should succeed");
    rt.parse(&toks).expect("parse should succeed")
}

fn root(ast: &Ast) -> &AstNode {
    ast.nodes().last().expect("ast is nonempty")
}

fn children<'a>(ast: &'a Ast, node: &'a AstNode) -> Vec<&'a AstNode> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("node has children")
        .iter()
        .map(|i| &ast.nodes()[i.as_u64().unwrap() as usize])
        .collect()
}

// ---------------------------------------------------------------------------
// Fragment presence
// ---------------------------------------------------------------------------

#[test]
fn graded_theory_tokens_exist() {
    let lang = full_language();
    for id in [
        "token.fat_arrow",
        "token.keyword.sort",
        "token.keyword.grade",
        "token.keyword.grades",
        "token.keyword.op",
        "token.keyword.handler",
        "token.keyword.rewrite",
    ] {
        assert!(lang.tokens.get_by_id(id).is_some(), "missing {id}");
    }
}

#[test]
fn graded_theory_grammar_rows_exist() {
    let lang = full_language();
    for id in [
        "grammar.sort.prefix",
        "grammar.grade.prefix",
        "grammar.op.prefix",
        "grammar.handler.prefix",
        "grammar.rewrite.prefix",
        "grammar.law.prefix",
    ] {
        assert!(lang.grammar.get_by_id(id).is_some(), "missing {id}");
    }
}

#[test]
fn graded_theory_ast_shapes_exist() {
    let lang = full_language();
    for (id, roles) in [
        ("ast.sort", vec!["name", "grades"]),
        ("ast.grade", vec!["name"]),
        ("ast.op", vec!["name", "signature"]),
        ("ast.handler", vec!["name", "impl", "grades"]),
        ("ast.rewrite", vec!["name", "source", "target"]),
        ("ast.law", vec!["name", "source", "target"]),
    ] {
        let shape = lang
            .ast
            .get_by_id(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        let declared: Vec<&str> = shape
            .children
            .as_ref()
            .expect("shape declares child roles")
            .iter()
            .map(|c| c.role.as_str())
            .collect();
        assert_eq!(declared, roles, "role mismatch for {id}");
    }
}

// ---------------------------------------------------------------------------
// Parse tests
// ---------------------------------------------------------------------------

#[test]
fn fat_arrow_lexes_before_bind() {
    // `=>` must lex as `token.fat_arrow`, not `=` (`token.bind`) + `>`.
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("a => b")).expect("lex");
    let kinds: Vec<&str> = toks.0.iter().map(|t| t.kind.as_str()).collect();
    assert!(kinds.contains(&"token.fat_arrow"), "tokens: {kinds:?}");
}

#[test]
fn sort_declaration_parses() {
    let ast = parse("sort Number grades Addable");
    let r = root(&ast);
    assert_eq!(r.kind, "sort");
    assert_eq!(r.payload.get("name").unwrap(), "Number");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].kind, "var");
    assert_eq!(cs[0].payload, serde_json::json!("Number"));
    assert_eq!(cs[1].payload, serde_json::json!("Addable"));
}

#[test]
fn parameterized_sort_parses() {
    // The parenthesized parameter form: `sort Vec(T, n: Nat)` parses the
    // head as a `call` whose args carry the parameters.
    let ast = parse("sort Vec(T, n: Nat) grades Lengthable(n)");
    let r = root(&ast);
    assert_eq!(r.kind, "sort");
    assert_eq!(r.payload.get("name").unwrap(), "Vec");
    let cs = children(&ast, r);
    assert_eq!(cs[0].kind, "call");
    assert_eq!(cs[1].kind, "call");
}

#[test]
fn bare_sort_parses_without_grades() {
    let ast = parse("sort Number");
    let r = root(&ast);
    assert_eq!(r.kind, "sort");
    assert_eq!(children(&ast, r).len(), 1);
}

#[test]
fn grade_declaration_parses() {
    let ast = parse("grade Addable");
    let r = root(&ast);
    assert_eq!(r.kind, "grade");
    assert_eq!(r.payload.get("name").unwrap(), "Addable");
}

#[test]
fn op_declaration_parses() {
    let ast = parse("op add : (A, A) ~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "op");
    assert_eq!(r.payload.get("name").unwrap(), "add");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].payload, serde_json::json!("add"));
    assert_eq!(cs[1].kind, "function");
}

#[test]
fn op_with_grade_clause_parses() {
    let ast = parse("op add : (A, A) ~Addable~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "op");
    assert_eq!(r.payload.get("name").unwrap(), "add");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].payload, serde_json::json!("add"));
    assert_eq!(cs[1].kind, "graded_arrow");
}

#[test]
fn handler_declaration_parses() {
    let ast = parse("handler NumberHandler = () ~> Handler");
    let r = root(&ast);
    assert_eq!(r.kind, "handler");
    assert_eq!(r.payload.get("name").unwrap(), "NumberHandler");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].payload, serde_json::json!("NumberHandler"));
    assert_eq!(cs[1].kind, "function");
}

#[test]
fn rewrite_declaration_parses() {
    let ast = parse("rewrite for_to_iter : f => g");
    let r = root(&ast);
    assert_eq!(r.kind, "rewrite");
    assert_eq!(r.payload.get("name").unwrap(), "for_to_iter");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[1].payload, serde_json::json!("f"));
    assert_eq!(cs[2].payload, serde_json::json!("g"));
}

#[test]
fn law_declaration_parses_as_2cell() {
    let ast = parse("law add_zero : add(0, x) => x");
    let r = root(&ast);
    assert_eq!(r.kind, "law");
    assert_eq!(r.payload.get("name").unwrap(), "add_zero");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[0].payload, serde_json::json!("add_zero"));
    assert_eq!(cs[1].kind, "call");
    assert_eq!(cs[2].payload, serde_json::json!("x"));
}
// ---------------------------------------------------------------------------
// Canonical bracketed graded arrow `A ~[R]~> B` (ADR-0106 §17)
// ---------------------------------------------------------------------------

/// The bracketed spelling is the canonical one. It lexes as
/// `~` `[` … `]` `~>` and parses to the same `graded_arrow` node as the
/// legacy `~R~>` shorthand, with the grade as the bracketed list.
#[test]
fn canonical_bracketed_graded_arrow_parses() {
    let ast = parse("(A, A) ~[Addable]~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "graded_arrow");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 3, "graded arrow is ternary: [left, grade, right]");
    assert_eq!(cs[0].kind, "paren");
    assert_eq!(cs[1].kind, "array", "the bracketed grade is the grade list");
    assert_eq!(cs[2].payload, serde_json::json!("A"));
}

/// A multi-grade bracketed list parses the same way.
#[test]
fn canonical_bracketed_graded_arrow_accepts_a_grade_list() {
    let ast = parse("(A, A) ~[Addable, Comparable]~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "graded_arrow");
    let cs = children(&ast, r);
    assert_eq!(cs[1].kind, "array");
    assert_eq!(children(&ast, cs[1]).len(), 2, "two grades in the list");
}

/// A non-trivial (call-shaped) grade inside the brackets parses.
#[test]
fn canonical_bracketed_graded_arrow_accepts_a_call_grade() {
    let ast = parse("(A, B) ~[Lengthable(n)]~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "graded_arrow");
    let cs = children(&ast, r);
    assert_eq!(cs[1].kind, "array");
}

/// The bracketed and legacy spellings produce the same node kind and arity.
#[test]
fn bracketed_and_legacy_graded_arrows_agree() {
    let legacy = parse("(A, A) ~Addable~> A");
    let canonical = parse("(A, A) ~[Addable]~> A");
    let (rl, rc) = (root(&legacy), root(&canonical));
    assert_eq!(rl.kind, rc.kind);
    assert_eq!(children(&legacy, rl).len(), children(&canonical, rc).len());
    // Both are consumed as graded arrows with the same left/right operands.
    assert_eq!(children(&legacy, rl)[0].kind, children(&canonical, rc)[0].kind);
    assert_eq!(
        children(&legacy, rl)[2].payload,
        children(&canonical, rc)[2].payload
    );
}

/// An empty grade list is rejected rather than minting a grade-less arrow.
#[test]
fn canonical_bracketed_graded_arrow_rejects_empty_grade() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("(A) ~[]~> A")).expect("lex succeeds");
    let err = rt
        .parse(&toks)
        .expect_err("`~[]~>` must not parse as a graded arrow");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("empty grade list"),
        "diagnostic should name the empty grade list, got: {msg}",
    );
}

