//! ADR-0120 Phase 4 — dependent structure tests.
//!
//! Exercises the `Family(A)` / `Pi(A, f)` / `Sigma(A, f)` surface type
//! formers, dependent introduction rules, `data` family declarations,
//! reindexing-as-composition (`f >>> P`), and the `Σ_f ⊣ f* ⊣ Π_f`
//! adjunction law terms against the Handl-authored typecheck model.
//!
//! These tests pin *structural* support only: index definitional
//! equality, strict positivity, constructor argument checking, and
//! dependent elimination are documented limitations, not asserted here.

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
    language_from_yaml().expect("canonical language should assemble")
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

/// The dependent surface formers and their introduction rules all check
/// cleanly: `Family` accepts an index predicate arrow, `Pi` checks a
/// function against the dependent function type, `Sigma` checks a pair
/// literal componentwise, and `data` binds the family and constructors.
#[test]
fn dependent_surface_forms_typecheck() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "x : Nat = 5",
            "fam : Family(Nat) = n ~> n > 0",
            "mk : Pi(Nat, n ~> Nat) = n ~> n",
            "mk2 : Pi(Nat, n ~> Nat) = n ~> n + 1",
            "p : Sigma(Nat, n ~> Nat) = [3, 4]",
            "data Bool :: True | False",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "dependent surface forms should check cleanly, got {:?}",
        diags_of(&result)
    );
}

/// Reindexing needs no primitive: `f >>> P` is ordinary composition and
/// synthesizes a family over `f`'s source (the pullback `f* P`).
#[test]
fn reindexing_is_ordinary_composition() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "P : Family(Nat) = n ~> n > 0",
            "f = n ~> n + 1",
            "Q = f >>> P",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "f >>> P should typecheck as reindexing, got {:?}",
        diags_of(&result)
    );
}

/// The `Σ_f ⊣ f* ⊣ Π_f` adjunction laws are ordinary `~[2]~>` 2-cell
/// terms bound as arrows over their participants — the same mechanism as
/// every other law, no dedicated subsystem.
#[test]
fn adjunction_laws_are_2cell_terms() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "reindex = (f, P) ~> f >>> P",
            "sigma_unit = (Q, f, Sigma_f) ~> Q ~[2]~> (f >>> Sigma_f(Q))",
            "sigma_counit = (P, f, Sigma_f) ~> Sigma_f((f >>> P)) ~[2]~> P",
            "pi_unit = (P, f, Pi_f) ~> P ~[2]~> Pi_f((f >>> P))",
            "pi_counit = (Q, f, Pi_f) ~> (f >>> Pi_f(Q)) ~[2]~> Q",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "adjunction law terms should check cleanly, got {:?}",
        diags_of(&result)
    );
}

/// `handl-packages/type/dependent.hnl` — the packaged Phase-4 data — must
/// parse statement-by-statement and typecheck without errors.
#[test]
fn dependent_hnl_parses_and_typechecks() {
    let src = include_str!("../../../handl-packages/type/dependent.hnl");
    let stmts: Vec<&str> = src
        .lines()
        .map(|l| l.split_once("--").map(|(c, _)| c).unwrap_or(l))
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert!(
        stmts.len() >= 5,
        "dependent.hnl should carry the reindex arrow and four law terms"
    );
    let lang = canonical_lang();
    let ast = parse_block(&lang, &stmts);
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "dependent.hnl should typecheck cleanly, got {:?}",
        diags_of(&result)
    );
}
