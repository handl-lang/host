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

/// Transport consumes the `Path` witness bound by dependent
/// elimination: under the `VNil` arm `refine_scrutinee_` binds
/// `eq_n ↦ Path{var n, 0}` in `results`, and `eq_n # v` rewrites `v`'s
/// type `Vec(n + 0, Bool)` through that path to `Vec(0, Bool)` —
/// exactly what the motive requires. Under `VCons` the same term
/// transports along `Path{var n, ?k + 1}` to `Vec(?k + 1, Bool)`.
/// The `via` field on the transport node's recorded type keeps the
/// witness (proof-relevant — not erased).
#[test]
fn transport_consumes_branch_path_witness() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "id_vec : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Vec(n + 0, Bool))) = n ~> v ~> match v return (i, w) ~> Vec(i, Bool) with { VNil: eq_n # v, VCons(x, xs): eq_n # v }",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "eq_n # v should transport v along the branch's Path witness, got {:?}",
        diags_of(&result)
    );
    let witnessed = result.node_types.values().any(|t| {
        t.get("via")
            .and_then(|p| p.get("__tag__"))
            .map(|tag| tag == "Path")
            .unwrap_or(false)
    });
    assert!(
        witnessed,
        "the transported type should record its Path witness under `via`"
    );
}

/// `refl` is the identity path: `p : Path(2, 2) = refl` checks (the
/// endpoints unify), `p # v` transports `v` along it, and a bare `refl`
/// in transport position is the identity coercion.
#[test]
fn refl_is_the_identity_path() {
    let lang = canonical_lang();
    let ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "p : Path(2, 2) = refl",
            "w : Vec(2, Bool) = p # v",
            "w2 : Vec(2, Bool) = refl # v",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "refl should inhabit Path(2, 2) and transport identically, got {:?}",
        diags_of(&result)
    );
}

/// The eliminators reject decided non-paths and absent paths:
/// `5 # v` and `v # v` are not transports over a `Path`, and `refl`
/// cannot prove a heterogeneous `Path(2, 3)`.
#[test]
fn transport_rejects_non_path_and_absent_paths() {
    let lang = canonical_lang();
    let decl = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";

    let num_path = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Vec(2, Bool) = 5 # v",
        ],
    );
    let result = typecheck(&lang, num_path);
    assert!(
        has_error(&result),
        "a number is not a path — 5 # v should be rejected"
    );

    let vec_path = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Vec(2, Bool) = v # v",
        ],
    );
    let result = typecheck(&lang, vec_path);
    assert!(
        has_error(&result),
        "a Vec is not a path — v # v should be rejected"
    );

    let unbound = parse_block(
        &lang,
        &[
            decl,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Vec(2, Bool) = nosuchpath # v",
        ],
    );
    let result = typecheck(&lang, unbound);
    assert!(
        has_error(&result),
        "an absent path should be rejected — nosuchpath # v"
    );

    let bad_refl = parse_block(&lang, &[decl, "bad : Path(2, 3) = refl"]);
    let result = typecheck(&lang, bad_refl);
    assert!(
        has_error(&result),
        "refl proves only a reflexive Path — Path(2, 3) should be rejected"
    );
}

/// The `j` eliminator consumes a path with an explicit motive:
/// `j eq_n, (i, w) ~> Vec(i, Bool), v` checks `v` against the motive
/// applied at the path's source (`Vec(var n, Bool)`) and synthesizes
/// the motive at the target — `Vec(0, Bool)` under `VNil`,
/// `Vec(?k + 1, Bool)` under `VCons` — composing with `apply_motive_`.
///
/// The `j` prefix grammar exists (`grammar.j.prefix`, `ast.j`) but no
/// keyword lexer rule emits `token.keyword.j` yet, so the surface form
/// cannot be parsed — the test grafts `j` nodes onto the arena
/// directly, rewriting each `eq_n # v` transport arm into
/// `j eq_n, motive, v`.
#[test]
fn j_eliminates_branch_path_through_motive() {
    let lang = canonical_lang();
    let mut ast = parse_block(
        &lang,
        &[
            "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
            "id_vec : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Vec(n + 0, Bool))) = n ~> v ~> match v return (i, w) ~> Vec(i, Bool) with { VNil: eq_n # v, VCons(x, xs): eq_n # v }",
            "_motive := (i2, w2) ~> Vec(i2, Bool)",
        ],
    );
    // The motive `(i2, w2) ~> Vec(i2, Bool)`, parsed in the same block
    // so `Vec` resolves in module scope, shared by both `j` nodes.
    let motive_idx = ast
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == "function")
        .map(|(i, _)| i)
        .last()
        .expect("the _motive statement should contain a function node");
    // Rewrite each `transport` arm `{children: [path, value]}` into a
    // `j` node `{children: [path, motive, base]}`.
    let transport_idx: Vec<usize> = ast
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == "transport")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(transport_idx.len(), 2, "both arms should be transports");
    for i in transport_idx {
        let node = ast.get_mut(AstRef::new(i)).unwrap();
        let children = node.payload["children"].as_array().unwrap().clone();
        node.kind = "j".to_string();
        node.payload = json!({ "children": [children[0], motive_idx, children[1]] });
    }
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "j eq_n, motive, v should eliminate the branch's Path through the motive, got {:?}",
        diags_of(&result)
    );
}

/// Bounded nonlinear index solving: `c * X ≡ t` solves `X` by exact
/// division. The `Dbl` family's constructor lands at `Dbl(2 * n)` —
/// commutative canonicalization already normalizes it to
/// `Bin{mul, ?n, 2}` — so checking `MkDbl` against `Dbl(6)` solves
/// `?n := 3` through the product, and matching a `Dbl(6)` scrutinee
/// refines the arm under `?k := 3`. A non-divisible target (`Dbl(5)`)
/// has no Nat solution: the equation is a *decided* failure, not stuck.
#[test]
fn nonlinear_product_solves_by_exact_division() {
    let lang = canonical_lang();
    let decl = "data Dbl(n : Nat) :: MkDbl(x : Nat) : Dbl(2 * n)";

    let good = parse_block(
        &lang,
        &[
            decl,
            "d : Dbl(6) = MkDbl(7)",
            "r : Nat = match d { MkDbl(x): x }",
        ],
    );
    let result = typecheck(&lang, good);
    assert!(
        !has_error(&result),
        "2 * ?n ≡ 6 should solve ?n := 3 (in the annotation and per arm), got {:?}",
        diags_of(&result)
    );

    let bad = parse_block(&lang, &[decl, "d : Dbl(5) = MkDbl(7)"]);
    let result = typecheck(&lang, bad);
    assert!(
        has_error(&result),
        "2 * ?n ≡ 5 has no Nat solution — a decided failure, not stuck"
    );
}

/// The square and small-power cases: `X * X ≡ t` (structurally equal
/// operands) solves by integer square root, and `X ** k ≡ t` for a
/// literal exponent solves by integer root. `Sq(9)` gives `?n := 3`
/// and `Cube(8)` gives `?n := 2`; `Sq(8)` and `Cube(9)` have no integer
/// root and are decided failures.
#[test]
fn nonlinear_square_and_pow_solve_by_integer_root() {
    let lang = canonical_lang();
    let sq = "data Sq(n : Nat) :: MkSq(x : Nat) : Sq(n * n)";
    let cube = "data Cube(n : Nat) :: MkCube(x : Nat) : Cube(n ** 3)";

    let good = parse_block(
        &lang,
        &[sq, cube, "s : Sq(9) = MkSq(7)", "c : Cube(8) = MkCube(1)"],
    );
    let result = typecheck(&lang, good);
    assert!(
        !has_error(&result),
        "?n * ?n ≡ 9 and ?n ** 3 ≡ 8 should solve by integer root, got {:?}",
        diags_of(&result)
    );

    let bad_sq = parse_block(&lang, &[sq, "s : Sq(8) = MkSq(7)"]);
    let result = typecheck(&lang, bad_sq);
    assert!(
        has_error(&result),
        "?n * ?n ≡ 8 has no integer root — decided failure"
    );

    let bad_cube = parse_block(&lang, &[cube, "c : Cube(9) = MkCube(1)"]);
    let result = typecheck(&lang, bad_cube);
    assert!(
        has_error(&result),
        "?n ** 3 ≡ 9 has no integer root — decided failure"
    );
}

/// `Ctor: absurd` is the explicit dismissal of a decided-impossible arm:
/// `VNil : Vec(0, ?a)` can never produce a `Vec(2, Bool)` scrutinee, so
/// `VNil: absurd` is *discharged* — no unreachable-branch diagnostic —
/// while the reachable `VCons` arm checks normally and contributes the
/// join. `Dbl`'s constructor lands at `Dbl(2 * ?k)`, so on a `Dbl(5)`
/// scrutinee the arm is decided-impossible by the nonlinear solver and
/// `MkDbl(x): absurd` is accepted the same way.
#[test]
fn absurd_discharges_decided_impossible_arm() {
    let lang = canonical_lang();
    let vec = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";
    let dbl = "data Dbl(n : Nat) :: MkDbl(x : Nat) : Dbl(2 * n)";

    let ast = parse_block(
        &lang,
        &[
            vec,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "x : Bool = match v { VNil: absurd, VCons(h, t): h }",
        ],
    );
    let result = typecheck(&lang, ast);
    assert!(
        !has_error(&result),
        "VNil: absurd on a Vec(2) scrutinee should be discharged, got {:?}",
        diags_of(&result)
    );

    let nonlinear = parse_block(
        &lang,
        &[
            dbl,
            "f : Pi(Dbl(5), d ~> Nat) = d ~> match d { MkDbl(x): absurd, y: 0 }",
        ],
    );
    let result = typecheck(&lang, nonlinear);
    assert!(
        !has_error(&result),
        "2 * ?k ≡ 5 is decided-unsatisfiable — the absurd arm should be discharged, got {:?}",
        diags_of(&result)
    );
}

/// `absurd` is only justified on a decided-impossible arm: marking a
/// *reachable* arm `absurd` is rejected. `VCons` on a `Vec(2)` scrutinee
/// solves `?k + 1 ≡ 2`, `VNil` on `Vec(0)` unifies outright, and a
/// rigid-index scrutinee (`v : Vec(n + 0, Bool)` — `n ≡ 0` binds under
/// the arm rather than failing) is reachable too, so all three `absurd`
/// arms error as reachable.
#[test]
fn absurd_rejects_reachable_arm() {
    let lang = canonical_lang();
    let vec = "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)";

    let reachable = parse_block(
        &lang,
        &[
            vec,
            "v : Vec(2, Bool) = VCons(true, VCons(false, VNil))",
            "bad : Bool = match v { VCons(x, xs): absurd }",
        ],
    );
    let result = typecheck(&lang, reachable);
    assert!(
        has_error(&result),
        "VCons: absurd on a Vec(2) scrutinee is reachable — must be rejected"
    );

    let exact = parse_block(
        &lang,
        &[
            vec,
            "v0 : Vec(0, Bool) = VNil",
            "bad : Bool = match v0 { VNil: absurd }",
        ],
    );
    let result = typecheck(&lang, exact);
    assert!(
        has_error(&result),
        "VNil: absurd on a Vec(0) scrutinee is reachable — must be rejected"
    );

    let rigid = parse_block(
        &lang,
        &[
            vec,
            "f : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Bool)) = n ~> v ~> match v { VNil: absurd, VCons(x, xs): x }",
        ],
    );
    let result = typecheck(&lang, rigid);
    assert!(
        has_error(&result),
        "on Vec(n + 0) the VNil arm refines n ≡ 0 rather than failing — absurd must be rejected"
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
