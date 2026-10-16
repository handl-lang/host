//! ADR-0120 Phase 2 follow-up — type-level globular boundary checking
//! for `a >>>[k] b`.
//!
//! The `synth_axis_flow` judgment op in the typecheck model implements
//! `tgt_k(a) = src_k(b)` on type-level cells: `Arrow{inputs, grade,
//! output, dim}` is an n-cell (`synth_dim_arrow` records the literal
//! bracket dimension), `type.function` and its dependent kin are
//! 1-cells, and every other type is a degenerate cell — the mirror of
//! the runtime rule that a non-`Cell2` value is its own boundary.

use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstNode, AstRef, Language};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_engine_typecheck_handl::HandlTypecheckEngine;
use serde_json::json;

fn canonical_lang() -> Language {
    let lang = language_from_yaml().expect("canonical language should assemble");
    // Also install it process-wide: the HandlTypecheckEngine's HandlModule
    // bridge runtime resolves `default_language` lazily (ADR-0128 §0.8).
    let _ = handl_plugin_lib_handl::install_default_language(lang.clone());
    lang
}

/// Parse a single statement.
fn parse_one(lang: &Language, src: &str) -> Ast {
    let tokens = lex(lang, src).expect("lex should succeed");
    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    engine.parse(lang, &req).expect("parse should succeed")
}

/// Append all nodes of `other` to `base`, shifting child/body indices.
fn append_ast(base: &mut Ast, other: &Ast) {
    let offset = base.len();
    for node in other.nodes() {
        let mut payload = node.payload.clone();
        if let Some(children) = payload.get_mut("children").and_then(|c| c.as_array_mut()) {
            for child in children.iter_mut() {
                if let Some(idx) = child.as_u64() {
                    *child = json!(idx as usize + offset);
                }
            }
        }
        if let Some(body) = payload.get_mut("body").and_then(|b| b.as_u64()) {
            *payload.get_mut("body").unwrap() = json!(body as usize + offset);
        }
        base.push(AstNode {
            kind: node.kind.clone(),
            span: node.span,
            payload,
        });
    }
}

/// Parse several statements and combine them under a synthetic `block` root
/// so the model's `typecheck` sees them as one program.
fn parse_block(lang: &Language, stmts: &[&str]) -> Ast {
    let mut combined = Ast::new();
    let mut roots = Vec::new();
    for src in stmts {
        let ast = parse_one(lang, src);
        let offset = combined.len();
        append_ast(&mut combined, &ast);
        let root_idx = ast.len() - 1 + offset;
        let root = combined.get(AstRef::new(root_idx)).expect("root");
        if root.kind == "block" {
            for c in root
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .expect("block children")
            {
                roots.push(c.as_u64().unwrap() as usize);
            }
        } else {
            roots.push(root_idx);
        }
    }
    combined.push(AstNode::new("block").with_payload(json!({ "children": roots })));
    combined
}

fn typecheck(lang: &Language, ast: Ast) -> handl_plugin_engine_typecheck::TypecheckResult {
    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);
    engine
        .typecheck(lang, &req)
        .expect("model should return a result")
}

fn diags_of(result: &handl_plugin_engine_typecheck::TypecheckResult) -> Vec<String> {
    result
        .diagnostics
        .iter()
        .map(|d| format!("{}: {}", d.severity, d.message))
        .collect()
}

fn has_error(result: &handl_plugin_engine_typecheck::TypecheckResult) -> bool {
    result.diagnostics.iter().any(|d| d.severity == "error")
}

fn error_msgs(result: &handl_plugin_engine_typecheck::TypecheckResult) -> Vec<String> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| d.message.clone())
        .collect()
}

/// `f ~[2]~> g` then `α >>>[1] β` composes when the shared boundary
/// `tgt_1(α) = src_1(β)` unifies — here `g` is literally the same arrow
/// on both sides, so the 2-cells paste vertically like the runtime's
/// `Cell2(src_α, tgt_β)` composite.
#[test]
fn axis1_well_typed_2cell_composition() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "f = x ~> x + 1",
            "g = x ~> x + 2",
            "h = x ~> x + 3",
            "alpha = f ~[2]~> g",
            "beta = g ~[2]~> h",
            "gamma = alpha >>>[1] beta",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "tgt_1(alpha) = src_1(beta) = typeof(g) should unify, got {:?}",
        diags_of(&result)
    );
}

/// The `∘₁` composite is itself a 2-cell type, so a second `>>>[1]`
/// composition keeps checking boundaries.
#[test]
fn axis1_composite_recomposes() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "f = x ~> x + 1",
            "g = x ~> x + 2",
            "h = x ~> x + 3",
            "alpha = f ~[2]~> g",
            "beta = g ~[2]~> h",
            "gamma = alpha >>>[1] beta",
            "delta = gamma >>>[1] alpha",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "the Arrow composite should recompose along axis 1, got {:?}",
        diags_of(&result)
    );
}

/// `tgt_1(α) ≠ src_1(β)` rejects with a boundary diagnostic: `alpha`'s
/// target is `g : number → number` (a `type.function`) while `beta`'s
/// source is the string cell `"s"` — `type.function ≁ type.string`.
/// (The model's `unify` is shallow for tagged records — boundary
/// equality is at the type-tag level, same as the axis-0 rule.)
#[test]
fn axis1_boundary_mismatch_errors() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "f = x ~> x + 1",
            "g = x ~> x + 2",
            "alpha = f ~[2]~> g",
            "beta = \"s\" ~[2]~> g",
            "gamma = alpha >>>[1] beta",
        ],
    );
    let result = typecheck(&lang, ast);
    let errs = error_msgs(&result);
    assert!(
        errs.iter().any(|m| m.contains("boundary")),
        "expected a boundary-mismatch diagnostic, got {:?}",
        diags_of(&result)
    );
}

/// Schematic cells: `dim_arrow` binds unbound endpoint vars to fresh
/// metavariables, so `α >>>[1] β` over two schematic `~[2]~>` terms
/// composes — the boundary check *constrains* the metavariables (the
/// same way `f >>> g` solves its domain metas).
#[test]
fn axis1_schematic_cells_compose() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "alpha = f ~[2]~> g",
            "beta = h ~[2]~> j",
            "gamma = alpha >>>[1] beta",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "schematic 2-cells should compose — g and h metas unify, got {:?}",
        diags_of(&result)
    );
}

/// Degenerate cells: a non-morphism operand stands for both boundaries,
/// mirroring the runtime's non-`Cell2` rule — `3 >>>[1] 3` composes
/// (same boundary type) while `1 >>>[1] \"s\"` rejects.
#[test]
fn axis1_degenerate_cells_compose_and_mismatch() {
    let lang = canonical_lang();
    let ok = typecheck(&lang, parse_block(&lang, &["x = 3 >>>[1] 3"]));
    assert!(
        !has_error(&ok),
        "degenerate cells with equal boundaries should compose, got {:?}",
        diags_of(&ok)
    );

    let bad = typecheck(&lang, parse_block(&lang, &["x = 1 >>>[1] \"s\""]));
    let errs = error_msgs(&bad);
    assert!(
        errs.iter().any(|m| m.contains("boundary")),
        "number vs string boundaries should mismatch, got {:?}",
        diags_of(&bad)
    );
}

/// A 1-cell (a `type.function`) is a degenerate 2-cell at axis 1 — the
/// runtime treats a plain arrow value as its own boundary, so
/// `f >>>[1] g` composes iff `f ~ g` unifies.
#[test]
fn axis1_arrow_operands_are_degenerate_2cells() {
    let lang = canonical_lang();
    let ok = typecheck(
        &lang,
        parse_block(
            &lang,
            &["f = x ~> x + 1", "g = x ~> x + 2", "c = f >>>[1] g"],
        ),
    );
    assert!(
        !has_error(&ok),
        "equal arrow types should compose as degenerate cells, got {:?}",
        diags_of(&ok)
    );

    let bad = typecheck(
        &lang,
        parse_block(&lang, &["f = x ~> x + 1", "c = f >>>[1] \"s\""]),
    );
    assert!(
        has_error(&bad),
        "a num→num arrow vs a string cell should fail the boundary check, got {:?}",
        diags_of(&bad)
    );
}

/// `k >= dim` fails with the dimension diagnostic: the runtime lifts
/// every `>>>[k]` operand to dimension 2, so `>>>[2]` always violates
/// `k < dim` — even on genuine `Arrow`-typed cells.
#[test]
fn axis_at_dimension_errors() {
    let lang = canonical_lang();
    let result = typecheck(&lang, parse_block(&lang, &["x = 1 >>>[2] 1"]));
    let errs = error_msgs(&result);
    assert!(
        errs.iter()
            .any(|m| m.contains("dimension") && m.contains("> 2")),
        "expected the dimension diagnostic, got {:?}",
        diags_of(&result)
    );
}

/// `>>>[0]` (and plain `>>>`) are unchanged: the axis-0 rule
/// `unify(f.body, g.param)` synthesizes `type.function{param, body}`.
#[test]
fn axis0_matches_flow() {
    let lang = canonical_lang();
    let result = typecheck(
        &lang,
        parse_block(
            &lang,
            &[
                "f = x ~> x + 1",
                "g = x ~> x * 2",
                "a = f >>>[0] g",
                "b = f >>> g",
            ],
        ),
    );
    assert!(
        !has_error(&result),
        ">>>[0] and >>> should typecheck identically, got {:?}",
        diags_of(&result)
    );
}

/// A non-literal axis keeps the axis-0 approximation and warns — the
/// runtime resolves `k` dynamically and the 0-boundary is the only
/// statically-known bound.
#[test]
fn nonliteral_axis_warns_and_checks_axis0() {
    let lang = canonical_lang();
    let result = typecheck(
        &lang,
        parse_block(
            &lang,
            &[
                "k = 1",
                "f = x ~> x + 1",
                "g = x ~> x + 2",
                "c = f >>>[k] g",
            ],
        ),
    );
    assert!(
        !has_error(&result),
        "non-literal axis should not error, got {:?}",
        diags_of(&result)
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.severity == "warning" && d.message.contains("non-literal axis")),
        "expected the non-literal-axis warning, got {:?}",
        diags_of(&result)
    );
}
