//! ADR-0120 Phase 4 — dependent structure tests.
//!
//! Exercises the `Family(A)` / `Pi(A, f)` / `Sigma(A, f)` surface type
//! formers, dependent introduction rules, `data` family declarations,
//! reindexing-as-composition (`f >>> P`), and the `Σ_f ⊣ f* ⊣ Π_f`
//! adjunction law terms against the Handl-authored typecheck model.
//!
//! These tests pin *structural* support: index definitional equality on
//! computable terms (`Vec(1 + 1)` ≡ `Vec(2)`), constructor argument
//! checking through freshly instantiated `type.ctor_sig` signatures, and
//! constructor-pattern binding in `match`. Strict positivity is enforced
//! by the desugar native. *Dependent elimination* is covered by
//! `match_refines_scrutinee_index_per_branch` and friends below: each arm
//! unifies the constructor's instantiated result with the scrutinee's
//! family index — metavariable indices solve (linearly), rigid `var`
//! indices are rewritten through the branch-local `results` environment
//! with a `Path` witness bound as `eq_<name>`, and a decided literal
//! mismatch rejects the arm as unreachable.

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

/// Index terms compute: `1 + 1`, `2 - 0`, `2 * 1` all normalize to `2`,
/// so a literal `Vec(2, Bool)` accepts a value constructed at those
/// indices. Constructor uses are freshly instantiated, so two uses of
/// `VCons` do not share solved metavariables.
#[test]
fn dependent_indices_compute_definitionally() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "v1 : Vec(1 + 1, Bool) = VCons(true, VCons(false, VNil))",
            "v2 : Vec(2 - 0, Bool) = VCons(true, VCons(false, VNil))",
            "v3 : Vec(2 * 1, Bool) = VCons(true, VCons(false, VNil))",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "computable index terms should be definitionally equal, got {:?}",
        diags_of(&result)
    );
}

/// A `data`-declared family shadows the builtin spelling, so
/// `Succ(pred : Nat)` takes the declared `Nat` family — `Succ(Zero)`
/// checks and `Succ(3)` does not.
#[test]
fn dependent_constructors_check_arguments() {
    let lang = canonical_lang();
    let good = parse_block(
        &lang,
        &[
            "data Nat :: Zero | Succ(pred : Nat)",
            "x : Nat = Succ(Zero)",
            "y : Nat = Succ(Succ(Zero))",
        ],
    );
    let result = typecheck(&lang, good);
    assert!(
        !has_error(&result),
        "constructor applications at the right types should check, got {:?}",
        diags_of(&result)
    );

    let bad = parse_block(
        &lang,
        &["data Nat :: Zero | Succ(pred : Nat)", "x : Nat = Succ(3)"],
    );
    let result = typecheck(&lang, bad);
    assert!(
        has_error(&result),
        "Succ(3) should be rejected — pred : Nat, not Nat-number"
    );
}

/// Indexed constructors validate both the length index and the element
/// type: a 2-vector at `Vec(3, Bool)` and a string element at
/// `Vec(_, Bool)` are both rejected.
#[test]
fn dependent_constructors_reject_wrong_indices() {
    let lang = canonical_lang();
    let decl = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";

    let wrong_len = parse_block(
        &lang,
        &[decl, "v : Vec(3, Bool) = VCons(true, VCons(false, VNil))"],
    );
    let result = typecheck(&lang, wrong_len);
    assert!(
        has_error(&result),
        "a 2-element vector should not check at Vec(3, Bool)"
    );

    let wrong_elem = parse_block(&lang, &[decl, "v : Vec(2, Bool) = VCons(\"s\", VNil)"]);
    let result = typecheck(&lang, wrong_elem);
    assert!(
        has_error(&result),
        "a string element should not check at Vec(1, Bool)"
    );
}

/// Dependent unification recurses through `type.function`: a family
/// application in the codomain still gets computational index equality.
#[test]
fn dependent_family_unifies_inside_function() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "f : Nat ~> Vec(1 + 1, Bool) = n ~> VCons(true, VCons(false, VNil))",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "family_app in a codomain should unify by normalized indices, got {:?}",
        diags_of(&result)
    );
}

/// Constructor patterns in `match` bind their fields against the
/// instantiated constructor domains. On a non-indexed family there is
/// no index to refine; the dependent-elimination tests below cover the
/// indexed case.
#[test]
fn match_constructor_patterns_bind_arguments() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Nat :: Zero | Succ(pred : Nat)",
            "f = v ~> match v { Zero: Zero, Succ(p): p }",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "constructor patterns should check cleanly, got {:?}",
        diags_of(&result)
    );
}

/// Dependent elimination: `match` unifies each constructor's
/// instantiated result type with the scrutinee's family application
/// *inside that arm*. The scrutinee `v : Vec(n + 0, Bool)` carries a
/// rigid `var{n}` index (a compound index term evaluates `n` as a term
/// variable, bound symbolically by the outer `Pi`), so the `VNil` arm
/// carries the equation `n ≡ 0` — realized as a branch-local `results`
/// rewrite witnessed by a `Path` under `eq_n` — and the `VCons` arm
/// carries `n ≡ ?k + 1`. The `return (i, w) ~> Vec(i, Bool) with`
/// motive makes the refinement observable: `v`'s fixed type
/// `Vec(n + 0, Bool)` only checks at `Vec(0, Bool)`/`Vec(?k + 1, Bool)`
/// because `n` is rewritten under each arm. Without refinement both
/// bodies would fail (`var{n}` does not unify with `0` or `?k + 1`).
#[test]
fn match_refines_scrutinee_index_per_branch() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "id_vec : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Vec(n + 0, Bool))) = n ~> v ~> match v return (i, w) ~> Vec(i, Bool) with { VNil: v, VCons(x, xs): v }",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "motive-indexed match should refine the rigid scrutinee index per branch, got {:?}",
        diags_of(&result)
    );
}

/// Index equations solve through the linear arithmetic fragment:
/// matching `v : Vec(2, Bool)` with `VCons(x, xs) : Vec(?k + 1, ?a)`
/// solves `?k := 1`, so `xs` is a `Vec(1, Bool)` inside the arm — the
/// nested match on `xs` then solves `?j + 1 ≡ 1` for its own `VCons`
/// arm, leaving `ys : Vec(0, Bool)` and `y : Bool`.
#[test]
fn match_refinement_solves_linear_index() {
    let lang = canonical_lang();
    let decl = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";
    let ast = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "g : Bool = match v { VCons(x, xs): match xs { VCons(y, ys): y } }",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "?k + 1 ≡ 2 should solve ?k := 1 so the nested Vec(1) match refines, got {:?}",
        diags_of(&result)
    );
}

/// A constructor whose result index *cannot* equal the scrutinee's is
/// an unreachable arm: `VNil : Vec(0, ?a)` can never produce a
/// `Vec(2, Bool)` scrutinee, so the arm is rejected (and its body is
/// not typechecked). Likewise, under `xs : Vec(1, Bool)` (refined from
/// `?k + 1 ≡ 2`), an inner `VNil` arm is impossible — that rejection is
/// only reachable because the outer arm solved `?k := 1`; without
/// refinement `xs`'s index is an unsolved metavariable and the inner
/// `VNil` arm passes.
#[test]
fn match_rejects_unreachable_constructor_arm() {
    let lang = canonical_lang();
    let decl = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";

    let direct = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Bool = match v { VNil: false, VCons(x, xs): x }",
        ],
    );
    let result = typecheck(&lang, direct);
    assert!(
        has_error(&result),
        "a VNil arm on a Vec(2, Bool) scrutinee is unreachable and should be rejected"
    );

    let nested = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Bool = match v { VCons(x, xs): match xs { VNil: false, VCons(y, ys): y } }",
        ],
    );
    let result = typecheck(&lang, nested);
    assert!(
        has_error(&result),
        "under xs : Vec(1, Bool) the inner VNil arm is unreachable and should be rejected"
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
