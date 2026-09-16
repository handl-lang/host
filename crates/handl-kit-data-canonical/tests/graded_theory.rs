//! Tests for the graded-theory declaration surface syntax (ADR-0098/0100).
//!
//! The canonical kit recognizes four declaration forms — `sort`, `grade`,
//! `op`, and `handler` — plus the `~[N]~>` N-dimensional arrow
//! (`token.dim_open` / `token.dim_close`). `law`/`rewrite`/`axiom` are
//! removed; 2-cell rules are ordinary `~[2]~>` arrow bindings.

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
        "token.dim_open",
        "token.dim_close",
        "token.keyword.sort",
        "token.keyword.grade",
        "token.keyword.grades",
        "token.keyword.op",
        "token.keyword.handler",
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
        "grammar.dim_arrow.infix",
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
        ("ast.dim_arrow", vec!["source", "dim", "target"]),
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
#[ignore = "n-dim"]
fn op_with_dim_clause_parses() {
    let ast = parse("op add : (A, A) ~[1]~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "op");
    assert_eq!(r.payload.get("name").unwrap(), "add");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].payload, serde_json::json!("add"));
    assert_eq!(cs[1].kind, "dim_arrow");
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

// ---------------------------------------------------------------------------
// N-dimensional arrow `A ~[N]~> B` (ADR-0119)
// ---------------------------------------------------------------------------

/// A one-dimensional arrow parses to the `dim_arrow` AST shape.
#[test]
#[ignore = "n-dim"]
fn dim_arrow_parses() {
    let ast = parse("(A, A) ~[1]~> A");
    let r = root(&ast);
    assert_eq!(r.kind, "dim_arrow");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 3, "dim arrow is ternary: [source, dim, target]");
    assert_eq!(cs[0].kind, "paren");
    assert_eq!(
        cs[1].payload,
        serde_json::json!(1),
        "the dimension child is a number"
    );
    assert_eq!(cs[2].payload, serde_json::json!("A"));
}
// ---------------------------------------------------------------------------
// Dimension specialization `f<...>` (ADR-0106 §11)
// ---------------------------------------------------------------------------
//
// The canonical spelling writes the bracket adjacent to a name-like target,
// which is what separates it from the comparison operator `<`:
//
//   f<a>            dimension specialization
//   f < a           comparison
//   f<a             comparison (the bracket never closes)
//   f<(a >>> b)>    dimension whose payload is a parenthesized composition
//   (f<a>) >>> b    flow whose left operand is the dimension
//
// These run through the real lexer, so the *byte-span adjacency* rule is
// exercised end to end (hand-built token streams cannot prove that).

/// `f<a>` parses to a `dimension` node (covered raw in `plugin-engine-parse-pratt`)
/// and the desugar pass lowers it to the application `f(a)`: the returned AST
/// is post-desugar, so the node is a `call` with the same children.
#[test]
fn dimension_bracket_lowers_to_call_through_real_lexer() {
    let ast = parse("f<a>");
    let r = root(&ast);
    assert_eq!(
        r.kind, "call",
        "`f<a>` lowers to the application `f(a)` (ADR-0106 §11/§12)",
    );
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2, "[target, arg]");
    assert_eq!(cs[0].payload, serde_json::json!("f"));
    assert_eq!(cs[1].payload, serde_json::json!("a"));
}

/// The bracket must be adjacent: `f < a` stays a comparison.
#[test]
fn spaced_angle_bracket_is_a_comparison_through_real_lexer() {
    let ast = parse("f < a");
    assert_eq!(
        root(&ast).kind,
        "binary.lt",
        "`f < a` (spaced) is a comparison, not a dimension",
    );
}

/// An unclosed bracket stays a comparison.
#[test]
fn unclosed_angle_bracket_is_a_comparison_through_real_lexer() {
    let ast = parse("f<a");
    assert_eq!(root(&ast).kind, "binary.lt");
}

/// A parenthesized composition *inside* the bracket is a dimension whose
/// payload is the composition; the trailing-flow form is a different tree.
#[test]
fn dimension_payload_composition_vs_trailing_flow() {
    // `f<(a >>> b)>` — composition inside the dimension.
    let inside = parse("f<(a >>> b)>");
    let r = root(&inside);
    assert_eq!(r.kind, "call", "the dimension lowers to an application");
    let cs = children(&inside, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[1].kind, "paren", "the payload is the parenthesized flow");

    // `(f<a>) >>> b` — the dimension is the *left operand* of a flow.
    let after = parse("(f<a>) >>> b");
    let root_after = root(&after);
    assert_eq!(root_after.kind, "flow");
    let flow_children = children(&after, root_after);
    assert_eq!(flow_children[0].kind, "paren");
    let inner = children(&after, flow_children[0]);
    assert_eq!(
        inner[0].kind, "call",
        "the lowered dimension is the left operand of the flow",
    );
}

/// Multiple dimension arguments parse.
#[test]
fn dimension_bracket_takes_multiple_arguments_through_real_lexer() {
    let ast = parse("map<T, U>");
    let r = root(&ast);
    assert_eq!(r.kind, "call", "`map<T, U>` lowers to `map(T, U)`");
    assert_eq!(children(&ast, r).len(), 3, "[target, T, U]");
}

/// A dimension *evaluates as application*: `f<X>` means `f(X)` (ADR-0106
/// §11/§12). Here `inc<41>` applies the arrow `inc` to `41`.
#[test]
fn dimension_evaluates_as_application() {
    let rt = full_runtime();
    let src = "do { inc :: x ~> x + 1\n  inc<41> }";
    let toks = rt.lex(&Source::new(src)).expect("lex succeeds");
    let ast = rt.parse(&toks).expect("parse succeeds");
    assert!(
        ast.nodes().iter().any(|n| n.kind == "call"),
        "the dimension must have lowered to an application",
    );
    let v = rt.eval(&ast).expect("eval succeeds");
    // The runtime may represent the number bare or tagged; accept either.
    let as_f64 = if let Some(n) = v.as_f64() {
        n
    } else {
        v.get("value")
            .and_then(|inner| inner.as_f64())
            .unwrap_or_else(|| panic!("result is not numeric: {v:?}"))
    };
    assert_eq!(as_f64, 42.0, "`inc<41>` must apply `inc` to 41");
}

/// `f<X>` evaluates exactly like `f(X)` — the strongest form of the
/// lowering claim, checked against the same runtime for both spellings.
#[test]
fn dimension_evaluates_identically_to_the_call_form() {
    let rt = full_runtime();
    let eval = |src: &str| {
        let toks = rt.lex(&Source::new(src)).expect("lex");
        let ast = rt.parse(&toks).expect("parse");
        rt.eval(&ast).expect("eval")
    };
    let call = eval("do { inc :: x ~> x + 1; inc(41) }");
    let dim = eval("do { inc :: x ~> x + 1; inc<41> }");
    assert_eq!(dim, call, "`inc<41>` must evaluate exactly like `inc(41)`");
    let multi_call = eval("do { add :: (a, b) ~> a + b; add(1, 2) }");
    let multi_dim = eval("do { add :: (a, b) ~> a + b; add<1, 2> }");
    assert_eq!(multi_dim, multi_call, "multi-argument form agrees too");
}

/// An empty bracket is a parse error, not a grade-less dimension.
#[test]
fn empty_dimension_bracket_rejected_through_real_lexer() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("f<>")).expect("lex succeeds");
    let err = rt.parse(&toks).expect_err("`f<>` must not parse");
    assert!(
        format!("{err:?}").contains("empty bracket"),
        "diagnostic should name the empty bracket: {err:?}",
    );
}
