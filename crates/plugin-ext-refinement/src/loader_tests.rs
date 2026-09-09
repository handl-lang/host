//! Tests for the handl contract file loader (`loader.rs`).
//!
//! These verify that:
//! - A simple axiom file parses into the correct `Contract`.
//! - Preconditions produce conditional contracts.
//! - Termination effects are parsed correctly.
//! - The bundled standard axiom set loads with the correct count.
//! - A round-trip (parse -> registry -> `infer_refinement`) works and matches
//!   `minimal()` for inference.
//! - Error cases (missing guarantee, malformed type, unknown clause, etc.) are
//!   reported.

use handl_language::{ArrSource, Term};

use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::loader::{load_standard_axioms, parse_contracts};
use crate::types::{BaseTy, Pred, RefTy};

// ============================================================================
// Helpers
// ============================================================================

/// Convenience: native arr with no args.
fn arr(op: &str) -> Term {
    Term::Arr {
        op: op.to_string(),
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    }
}

/// The total number of axioms across the bundled `prim.hnl`, `list.hnl`,
/// `parse.hnl`, and `state.hnl`.
const STANDARD_AXIOM_COUNT: usize = 39;

// ============================================================================
// 1. Parse a simple axiom file -> correct Contract
// ============================================================================

#[test]
fn test_parse_simple_axiom() {
    let src = "axiom prim.add : (Int * Int) -> Int\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("prim.add", &ArrSource::Native)
        .expect("prim.add registered");
    assert_eq!(
        c.input,
        RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Int)),
            Box::new(RefTy::Plain(BaseTy::Int)),
        )
    );
    assert_eq!(c.output, RefTy::Plain(BaseTy::Int));
    assert_eq!(c.precondition, Pred::True);
    assert_eq!(c.guarantee, Pred::True);
    assert_eq!(c.termination_effect, None);
}

#[test]
fn test_parse_axiom_with_list_type() {
    let src = "axiom list.head : List a -> a\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("list.head", &ArrSource::Native)
        .expect("list.head registered");
    assert_eq!(
        c.input,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string()))))
    );
    assert_eq!(c.output, RefTy::Plain(BaseTy::Var("a".to_string())));
}

#[test]
fn test_parse_axiom_with_lengtheq_guarantee() {
    let src = "axiom prim.alloc : Int -> List a\n  guarantee: LengthEq(output, 0)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("prim.alloc", &ArrSource::Native)
        .expect("prim.alloc registered");
    assert_eq!(
        c.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
}

#[test]
fn test_parse_axiom_with_product_input() {
    let src = "axiom cons : (a * List a) -> List a\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("cons", &ArrSource::Native)
        .expect("cons registered");
    assert_eq!(
        c.input,
        RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Var("a".to_string()))),
            Box::new(RefTy::List(Box::new(RefTy::Plain(BaseTy::Var(
                "a".to_string()
            )))))
        )
    );
    assert_eq!(
        c.output,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string()))))
    );
}

#[test]
fn test_parse_comments_and_blank_lines_ignored() {
    let src = "-- a comment\n\naxiom id : a -> a\n  -- inline comment\n  guarantee: True\n\n-- trailing comment\n";
    let reg = parse_contracts(src).expect("must parse");
    assert!(reg.contains("id", &ArrSource::Native));
    assert_eq!(reg.len(), 1);
}

#[test]
fn test_parse_multiple_axioms() {
    let src = "\
axiom prim.add : (Int * Int) -> Int
  guarantee: True

axiom prim.compare : (a * b) -> Int
  guarantee: True

axiom prim.alloc : Int -> List a
  guarantee: LengthEq(output, 0)
";
    let reg = parse_contracts(src).expect("must parse");
    assert_eq!(reg.len(), 3);
    assert!(reg.contains("prim.add", &ArrSource::Native));
    assert!(reg.contains("prim.compare", &ArrSource::Native));
    assert!(reg.contains("prim.alloc", &ArrSource::Native));
}

// ============================================================================
// 2. Parse with precondition -> conditional contract
// ============================================================================

#[test]
fn test_parse_axiom_with_precondition() {
    let src = "\
axiom remove_one : List a -> List a
  requires: LengthLe(input, 10)
  guarantee: LengthLe(output, input)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("remove_one", &ArrSource::Native)
        .expect("remove_one registered");
    assert_eq!(
        c.precondition,
        Pred::LengthLe("input".to_string(), "10".to_string())
    );
    assert_eq!(
        c.guarantee,
        Pred::LengthLe("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_axiom_precondition_defaults_to_true() {
    // No `requires:` clause -> precondition should default to True.
    let src = "axiom id : a -> a\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.precondition, Pred::True);
}

#[test]
fn test_parse_axiom_with_conjunctive_guarantee() {
    let src = "\
axiom filter : List a -> List a
  guarantee: AllSatisfy(output, predicate) && LengthLe(output, input)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("filter", &ArrSource::Native).unwrap();
    let expected = Pred::And(
        Box::new(Pred::AllSatisfy(
            "output".to_string(),
            "predicate".to_string(),
        )),
        Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
    );
    assert_eq!(c.guarantee, expected);
}

#[test]
fn test_parse_axiom_with_implication_guarantee() {
    let src = "\
axiom cond : List a -> List a
  guarantee: Sorted(input) ==> Sorted(output)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cond", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::Implies(
            Box::new(Pred::Sorted("input".to_string())),
            Box::new(Pred::Sorted("output".to_string())),
        )
    );
}

// ============================================================================
// 3. Parse with termination effect -> correct effect
// ============================================================================

#[test]
fn test_parse_axiom_with_termination_decrements_length() {
    let src = "\
axiom consume_one : List a -> List a
  guarantee: True
  termination: decrements_length
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("consume_one", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("decrements_length"));
}

#[test]
fn test_parse_axiom_with_termination_decrements_counter() {
    let src = "\
axiom decrement : Int -> Int
  guarantee: True
  termination: decrements_counter
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("decrement", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("decrements_counter"));
}

#[test]
fn test_parse_axiom_with_termination_constant() {
    let src = "\
axiom id : a -> a
  guarantee: True
  termination: constant
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("constant"));
}

#[test]
fn test_parse_axiom_with_termination_none() {
    let src = "\
axiom sort : List a -> List a
  guarantee: Sorted(output)
  termination: none
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("sort", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("none"));
    assert_eq!(c.guarantee, Pred::Sorted("output".to_string()));
}

#[test]
fn test_parse_axiom_termination_defaults_to_none() {
    // No `termination:` clause -> termination_effect should be None.
    let src = "axiom id : a -> a\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect, None);
}

// ============================================================================
// 4. Parse all standard axioms -> registry has correct count
// ============================================================================

#[test]
fn test_load_standard_axioms_count() {
    let reg = load_standard_axioms();
    assert_eq!(
        reg.len(),
        STANDARD_AXIOM_COUNT,
        "standard axiom set must contain exactly {} axioms",
        STANDARD_AXIOM_COUNT
    );
    assert!(!reg.is_empty());
}

#[test]
fn test_load_standard_axioms_has_prim_atoms() {
    let reg = load_standard_axioms();
    assert!(reg.contains("prim.add", &ArrSource::Native));
    assert!(reg.contains("prim.compare", &ArrSource::Native));
    assert!(reg.contains("prim.alloc", &ArrSource::Native));
}

#[test]
fn test_load_standard_axioms_has_list_atoms() {
    let reg = load_standard_axioms();
    assert!(reg.contains("list.cons", &ArrSource::Native));
    assert!(reg.contains("list.head", &ArrSource::Native));
    assert!(reg.contains("list.tail", &ArrSource::Native));
    assert!(reg.contains("list.is_empty", &ArrSource::Native));
    assert!(reg.contains("cons", &ArrSource::Native));
    assert!(reg.contains("id", &ArrSource::Native));
    assert!(reg.contains("fn.apply", &ArrSource::Native));
    assert!(reg.contains("filter_step", &ArrSource::Native));
    assert!(reg.contains("fold_step", &ArrSource::Native));
}

#[test]
fn test_load_standard_axioms_prim_alloc_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("prim.alloc", &ArrSource::Native)
        .expect("prim.alloc present");
    assert_eq!(
        c.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
    assert_eq!(c.precondition, Pred::True);
}

// ============================================================================
// 4b. Enriched axioms: conditional contracts with real semantic content
// ============================================================================

#[test]
fn test_load_standard_axioms_prim_compare_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("prim.compare", &ArrSource::Native)
        .expect("prim.compare present");
    // Enriched: prim.compare guarantees Le(output, 1).
    assert_eq!(c.guarantee, Pred::Le("output".to_string(), "1".to_string()));
    assert_eq!(c.precondition, Pred::True);
}

#[test]
fn test_load_standard_axioms_cons_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("cons", &ArrSource::Native)
        .expect("cons present");
    // Enriched: cons has a conditional contract.
    // requires: Sorted(tail) && Le(head, head_of_tail)
    // guarantee: Sorted(output) && LengthPlusOne(output, tail) && IsPerm(output, output)
    // (Parser is left-associative: A && B && C -> And(And(A, B), C))
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthPlusOne(
                    "output".to_string(),
                    "tail".to_string()
                )),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_list_cons_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("list.cons", &ArrSource::Native)
        .expect("list.cons present");
    // Same conditional contract as cons.
    // (Parser is left-associative: A && B && C -> And(And(A, B), C))
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthPlusOne(
                    "output".to_string(),
                    "tail".to_string()
                )),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_list_tail_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("list.tail", &ArrSource::Native)
        .expect("list.tail present");
    // Enriched: list.tail preserves sortedness and decrements length.
    // requires: Sorted(input)
    // guarantee: Sorted(output) && LengthSubOne(output, input) && IsPerm(output, output)
    // (Parser is left-associative: A && B && C -> And(And(A, B), C))
    assert_eq!(c.precondition, Pred::Sorted("input".to_string()));
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthSubOne(
                    "output".to_string(),
                    "input".to_string()
                )),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_filter_step_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("filter_step", &ArrSource::Native)
        .expect("filter_step present");
    // Enriched: filter_step guarantees AllSatisfy && LengthLe.
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::AllSatisfy(
                "output".to_string(),
                "predicate".to_string()
            )),
            Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
        )
    );
    assert_eq!(c.precondition, Pred::True);
}

// ============================================================================
// 4c. New predicate parsing
// ============================================================================

#[test]
fn test_parse_le_predicate() {
    let src = "axiom cmp : (Int * Int) -> Int\n  guarantee: Le(output, 1)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cmp", &ArrSource::Native).unwrap();
    assert_eq!(c.guarantee, Pred::Le("output".to_string(), "1".to_string()));
}

#[test]
fn test_parse_head_of_predicate() {
    let src = "axiom hd : List a -> a\n  guarantee: HeadOf(output, input)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("hd", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::HeadOf("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_length_plus_one_predicate() {
    let src = "axiom cons : (a * List a) -> List a\n  guarantee: LengthPlusOne(output, tail)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cons", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::LengthPlusOne("output".to_string(), "tail".to_string())
    );
}

#[test]
fn test_parse_length_sub_one_predicate() {
    let src = "axiom tl : List a -> List a\n  guarantee: LengthSubOne(output, input)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("tl", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::LengthSubOne("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_conditional_cons_contract() {
    let src = "\
axiom cons : (a * List a) -> List a
  requires: Sorted(tail) && Le(head, head_of_tail)
  guarantee: Sorted(output) && LengthPlusOne(output, tail)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cons", &ArrSource::Native).unwrap();
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::Sorted("output".to_string())),
            Box::new(Pred::LengthPlusOne(
                "output".to_string(),
                "tail".to_string()
            )),
        )
    );
}

#[test]
fn test_from_axioms_matches_load_standard_axioms() {
    // ContractRegistry::from_axioms() must delegate to load_standard_axioms().
    let via_from = ContractRegistry::from_axioms();
    let via_load = load_standard_axioms();
    assert_eq!(via_from.len(), via_load.len());
}

// ============================================================================
// 5. Round-trip: parse -> registry -> infer_refinement works
// ============================================================================

#[test]
fn test_round_trip_inference_prim_alloc() {
    let reg = load_standard_axioms();
    let g = arr("prim.alloc");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    // prim.alloc has an unconditional precondition, so its guarantee is used.
    assert_eq!(
        result.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
}

#[test]
fn test_round_trip_inference_prim_add() {
    let reg = load_standard_axioms();
    let g = arr("prim.add");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    assert_eq!(result.guarantee, Pred::True);
}

#[test]
fn test_round_trip_inference_list_head() {
    let reg = load_standard_axioms();
    let g = arr("list.head");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    assert_eq!(result.guarantee, Pred::True);
    assert_eq!(
        result.input,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string()))))
    );
    assert_eq!(result.output, RefTy::Plain(BaseTy::Var("a".to_string())));
}

#[test]
fn test_from_axioms_matches_minimal_for_inference() {
    // The from_axioms() registry must produce the same inference results as
    // minimal() for the prim.alloc atom (the rule's requirement #4).
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let g = arr("prim.alloc");
    let res_axioms = infer_refinement(&g, &reg_axioms).expect("axioms inference");
    let res_minimal = infer_refinement(&g, &reg_minimal).expect("minimal inference");
    assert_eq!(res_axioms.guarantee, res_minimal.guarantee);
    assert_eq!(res_axioms.input, res_minimal.input);
    assert_eq!(res_axioms.output, res_minimal.output);
}

#[test]
fn test_from_axioms_matches_minimal_for_prim_add() {
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let g = arr("prim.add");
    let res_axioms = infer_refinement(&g, &reg_axioms).expect("axioms inference");
    let res_minimal = infer_refinement(&g, &reg_minimal).expect("minimal inference");
    assert_eq!(res_axioms.guarantee, res_minimal.guarantee);
}

#[test]
fn test_from_axioms_and_minimal_have_same_count() {
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();
    assert_eq!(reg_axioms.len(), reg_minimal.len());
}

// ============================================================================
// 6. Error cases
// ============================================================================

#[test]
fn test_error_missing_guarantee() {
    let src = "axiom id : a -> a\n";
    let err = parse_contracts(src).expect_err("must error on missing guarantee");
    assert!(
        err.contains("guarantee"),
        "error should mention guarantee: {}",
        err
    );
}

#[test]
fn test_error_missing_arrow() {
    let src = "axiom id : a\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on missing arrow");
    assert!(
        err.contains("->") || err.contains("arrow"),
        "error should mention the arrow: {}",
        err
    );
}

#[test]
fn test_error_missing_colon() {
    let src = "axiom id a -> a\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on missing colon");
    assert!(
        err.contains(":") || err.contains("colon"),
        "error should mention the colon: {}",
        err
    );
}

#[test]
fn test_error_malformed_type() {
    // `*` without a closing `)`.
    let src = "axiom bad : (Int * Int -> Int\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on malformed type");
    assert!(!err.is_empty(), "error message must be non-empty: {}", err);
}

#[test]
fn test_error_unknown_clause() {
    let src = "axiom id : a -> a\n  guarantee: True\n  bogus: something\n";
    let err = parse_contracts(src).expect_err("must error on unknown clause");
    assert!(
        err.contains("bogus") || err.contains("unknown"),
        "error should mention the unknown clause: {}",
        err
    );
}

#[test]
fn test_error_unknown_termination_effect() {
    let src = "axiom id : a -> a\n  guarantee: True\n  termination: explodes\n";
    let err = parse_contracts(src).expect_err("must error on bad termination");
    assert!(
        err.contains("termination") || err.contains("explodes"),
        "error should mention the bad effect: {}",
        err
    );
}

#[test]
fn test_error_unknown_predicate() {
    let src = "axiom id : a -> a\n  guarantee: Mystery(output)\n";
    let err = parse_contracts(src).expect_err("must error on unknown predicate");
    assert!(
        err.contains("Mystery") || err.contains("unknown predicate"),
        "error should mention the unknown predicate: {}",
        err
    );
}

#[test]
fn test_error_top_level_non_axiom_line() {
    let src = "hello world\n";
    let err = parse_contracts(src).expect_err("must error on non-axiom top-level line");
    assert!(
        err.contains("axiom") || err.contains("unexpected"),
        "error should mention the unexpected line: {}",
        err
    );
}

#[test]
fn test_error_empty_guarantee_value() {
    // `guarantee:` with no value -- the predicate parser should reject empty input.
    let src = "axiom id : a -> a\n  guarantee:\n";
    let err = parse_contracts(src).expect_err("must error on empty guarantee");
    assert!(!err.is_empty(), "error message must be non-empty: {}", err);
}

// ============================================================================
// Module system tests (ADR-0069 §4–§5)
// ============================================================================
//
// These tests verify the pluggable resolver abstraction, the two-cache
// ModuleLoader, and the three normalization strategies ported from the PoC
// (research/arrow-calculus-kernel-poc/src/modules.rs) into the production
// crate.

use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use crate::arrow_loader::{
    CachingResolver, ChainResolver, FileResolver, LoadError, LoadedModule, MemoryResolver,
    ModuleLoader, ModuleResolver, ResolveErrorKind, flatten_program, has_unresolved_apps,
    normalize_compile_time,
};
use crate::arrow_parser::{ExportDecl, ImportDecl, parse_arrow_module};

// --- CountingResolver (for cache-hit verification) ---

/// A wrapper resolver that counts how many times `resolve` is called.
/// Used to verify that `CachingResolver` and `ModuleLoader` caches prevent
/// redundant resolution.
struct CountingResolver {
    inner: Box<dyn ModuleResolver>,
    count: Rc<Cell<usize>>,
}

impl CountingResolver {
    fn new(inner: Box<dyn ModuleResolver>) -> Self {
        CountingResolver {
            inner,
            count: Rc::new(Cell::new(0)),
        }
    }

    fn new_with_counter(inner: Box<dyn ModuleResolver>) -> (Self, Rc<Cell<usize>>) {
        let count = Rc::new(Cell::new(0));
        let r = CountingResolver {
            inner,
            count: count.clone(),
        };
        (r, count)
    }
}

impl ModuleResolver for CountingResolver {
    fn resolve(
        &self,
        path: &str,
    ) -> Result<crate::arrow_loader::ResolvedModule, crate::arrow_loader::ResolveError> {
        self.count.set(self.count.get() + 1);
        self.inner.resolve(path)
    }

    fn origin(&self) -> &'static str {
        self.inner.origin()
    }
}

// --- Test data builder ---

/// Build a `MemoryResolver` with the standard test module set.
fn build_test_memory_resolver() -> MemoryResolver {
    let mut m = MemoryResolver::new();
    // collection/insert — a single named arrow.
    m.insert("collection/insert", "insert = arr(\"compare\")");
    // collection/sort — imports insert, defines sort.
    m.insert(
        "collection/sort",
        "import insert from \"collection/insert\"\nsort = arr(\"sort\")",
    );
    // collection — multi-name file (record arrow).
    m.insert(
        "collection",
        "cons = arr(\"list.cons\")\nhead = arr(\"list.head\")\nsort = arr(\"sort\")",
    );
    // main — imports sort, uses it.
    m.insert("main", "import sort from \"collection/sort\"\nmain = sort");
    // reexport — export * from "collection".
    m.insert("reexport", "export * from \"collection\"");
    // cycle modules.
    m.insert("cycle_a", "import b from \"cycle_b\"\nx = arr(\"id\")");
    m.insert("cycle_b", "import a from \"cycle_a\"\ny = arr(\"id\")");
    m
}

// ============================================================================
// Section 1: Resolvers
// ============================================================================

#[test]
fn test_memory_resolver_resolves() {
    let mem = build_test_memory_resolver();
    let r = mem.resolve("collection/sort");
    match r {
        Ok(m) => {
            assert!(
                m.source.contains("import insert"),
                "MemoryResolver should return source with import: {}",
                m.source
            );
            assert_eq!(mem.origin(), "memory");
        }
        Err(_) => panic!("MemoryResolver should resolve collection/sort"),
    }
}

#[test]
fn test_memory_resolver_not_found() {
    let mem = build_test_memory_resolver();
    match mem.resolve("missing") {
        Ok(_) => panic!("MemoryResolver should return NotFound for missing"),
        Err(e) => assert!(matches!(e.kind, ResolveErrorKind::NotFound)),
    }
}

#[test]
fn test_chain_resolver_finds_memory() {
    let mut mem = MemoryResolver::new();
    mem.insert("collection/sort", "sort = arr(\"id\")");
    let mut resolvers: Vec<Box<dyn ModuleResolver>> = Vec::new();
    resolvers.push(Box::new(mem));
    resolvers.push(Box::new(FileResolver::new(PathBuf::from(
        "/tmp/handl_test_nonexistent",
    ))));
    let chain = ChainResolver::new(resolvers);

    match chain.resolve("collection/sort") {
        Ok(m) => assert_eq!(m.source, "sort = arr(\"id\")"),
        Err(_) => panic!("ChainResolver should find memory module"),
    }
    match chain.resolve("totally/missing") {
        Ok(_) => panic!("ChainResolver should return NotFound for missing"),
        Err(e) => assert!(matches!(e.kind, ResolveErrorKind::NotFound)),
    }
    assert_eq!(chain.origin(), "chain");
}

#[test]
fn test_caching_resolver_caches() {
    let mut mem = MemoryResolver::new();
    mem.insert("collection/sort", "sort = arr(\"id\")");
    let (counting, counter) = CountingResolver::new_with_counter(Box::new(mem));
    let cache = CachingResolver::new(Box::new(counting));

    let r1 = cache.resolve("collection/sort");
    let r2 = cache.resolve("collection/sort");
    assert!(r1.is_ok() && r2.is_ok(), "CachingResolver returns Ok twice");
    assert_eq!(
        r1.unwrap().source,
        r2.unwrap().source,
        "CachingResolver returns same content"
    );
    // The inner resolver should only be called once (cache hit on second).
    assert_eq!(
        counter.get(),
        1,
        "CachingResolver should call inner resolve only once"
    );
}

// ============================================================================
// Section 2: ModuleLoader (Strategy C — two caches)
// ============================================================================

#[test]
fn test_module_loader_load() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match loader.load("collection/sort") {
        Ok(loaded) => {
            // Should have arrows: "sort" (and possibly "insert" if the parser
            // treats the import as a binding — but our parser strips import lines).
            assert!(
                loaded.arrows.contains_key("sort"),
                "loaded module should have 'sort' arrow: {:?}",
                loaded.arrows.keys()
            );
        }
        Err(e) => panic!("load(collection/sort) should succeed: {}", e),
    }
}

#[test]
fn test_module_loader_cache_hit() {
    let mem = build_test_memory_resolver();
    let (counting, counter) = CountingResolver::new_with_counter(Box::new(mem));
    let loader = ModuleLoader::new(Box::new(counting));

    // First load.
    let _ = loader.load("collection/sort").expect("first load");
    let count_before = counter.get();

    // Second load — should be a cache hit (no new resolve call).
    let _ = loader.load("collection/sort").expect("second load");
    let count_after = counter.get();

    assert_eq!(
        count_before, count_after,
        "cache hit: second load should not call resolve"
    );
}

#[test]
fn test_module_loader_public_names_all() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match loader.public_names("collection") {
        Ok(names) => {
            assert!(
                names.len() == 3
                    && names.contains(&"cons".to_string())
                    && names.contains(&"head".to_string())
                    && names.contains(&"sort".to_string()),
                "public_names(collection) should be [cons, head, sort]: {:?}",
                names
            );
        }
        Err(e) => panic!("public_names(collection) should succeed: {}", e),
    }
}

#[test]
fn test_module_loader_public_names_with_exports() {
    let mut mem = MemoryResolver::new();
    mem.insert(
        "mod_with_exports",
        "export sort, cons\nsort = arr(\"sort\")\ncons = arr(\"list.cons\")\nhidden = arr(\"id\")",
    );
    let loader = ModuleLoader::new(Box::new(mem));

    match loader.public_names("mod_with_exports") {
        Ok(names) => {
            assert_eq!(names.len(), 2, "should export exactly 2 names: {:?}", names);
            assert!(names.contains(&"sort".to_string()));
            assert!(names.contains(&"cons".to_string()));
            assert!(!names.contains(&"hidden".to_string()));
        }
        Err(e) => panic!("public_names should succeed: {}", e),
    }
}

#[test]
fn test_module_loader_load_arrow() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match loader.load_arrow("collection/sort", "sort") {
        Ok(scheme) => {
            // The sort arrow should be an Arr node.
            assert!(
                matches!(scheme, Term::Arr { .. }),
                "load_arrow should return a Term::Arr"
            );
        }
        Err(e) => panic!("load_arrow should succeed: {}", e),
    }
}

#[test]
fn test_module_loader_load_arrow_single() {
    let mut mem = MemoryResolver::new();
    mem.insert("single", "arr(\"id\")");
    let loader = ModuleLoader::new(Box::new(mem));

    // Single-expression file: any name should return the arrow.
    match loader.load_arrow("single", "anything") {
        Ok(scheme) => {
            assert!(
                matches!(scheme, Term::Arr { op, .. } if op == "id"),
                "load_arrow on single-expr module should return the arrow"
            );
        }
        Err(e) => panic!("load_arrow on single-expr module should succeed: {}", e),
    }
}

// ============================================================================
// Section 3: Strategy A — normalize_compile_time
// ============================================================================

#[test]
fn test_normalize_compile_time_no_module_ref() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match normalize_compile_time("main", &loader) {
        Ok(normalized) => {
            // No module_ref nodes should remain.
            let any_ref = normalized
                .values()
                .any(|s| crate::arrow_loader::contains_module_ref(s));
            assert!(!any_ref, "Strategy A: no module_ref should remain");
        }
        Err(e) => panic!("normalize_compile_time(main) should succeed: {}", e),
    }
}

#[test]
fn test_normalize_compile_time_has_project() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match normalize_compile_time("main", &loader) {
        Ok(normalized) => {
            // Strategy A keeps projections: the "main" arrow should have a
            // project node (from the import of "sort").
            let main = normalized.get("main");
            assert!(main.is_some(), "normalized module should have 'main' arrow");
            if let Some(scheme) = main {
                assert!(
                    crate::arrow_loader::contains_project(scheme),
                    "Strategy A: project nodes should remain in main arrow"
                );
            }
        }
        Err(e) => panic!("normalize_compile_time should succeed: {}", e),
    }
}

// ============================================================================
// Section 4: Strategy B — flatten_program
// ============================================================================

#[test]
fn test_flatten_program_no_unresolved_apps() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match flatten_program("main", &loader) {
        Ok(flat) => {
            // After flattening, there should be no unresolved App nodes.
            // All known names: the entry's arrows + all transitive imports.
            let known: HashSet<String> =
                vec!["main".to_string(), "sort".to_string(), "insert".to_string()]
                    .into_iter()
                    .collect();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "flatten_program: no unresolved App nodes should remain"
            );
        }
        Err(e) => panic!("flatten_program(main) should succeed: {}", e),
    }
}

#[test]
fn test_flatten_program_no_module_ref() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match flatten_program("main", &loader) {
        Ok(flat) => {
            assert!(
                !crate::arrow_loader::contains_module_ref(&flat),
                "flatten_program: no module_ref should remain"
            );
        }
        Err(e) => panic!("flatten_program should succeed: {}", e),
    }
}

// ============================================================================
// Section 5: Cycle detection
// ============================================================================

#[test]
fn test_cycle_detection_normalize() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match normalize_compile_time("cycle_a", &loader) {
        Ok(_) => panic!("cycle_a normalization should return Cycle error"),
        Err(e) => {
            assert!(
                matches!(e, LoadError::Cycle(_)),
                "cycle_a should return Cycle error, got: {}",
                e
            );
        }
    }
}

#[test]
fn test_cycle_detection_flatten() {
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match flatten_program("cycle_a", &loader) {
        Ok(_) => panic!("cycle_a flatten should return Cycle error"),
        Err(e) => {
            assert!(
                matches!(e, LoadError::Cycle(_)),
                "cycle_a flatten should return Cycle error, got: {}",
                e
            );
        }
    }
}

// ============================================================================
// Section 6: Import/export parsing
// ============================================================================

#[test]
fn test_parse_arrow_module_simple_import() {
    let src = "import sort from \"collection/sort\"\nmain = arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.imports.len(), 1, "should have 1 import");
    assert_eq!(parsed.imports[0].path, "collection/sort");
    assert_eq!(parsed.imports[0].names.len(), 1);
    assert_eq!(
        parsed.imports[0].names[0],
        ("sort".to_string(), "sort".to_string())
    );
}

#[test]
fn test_parse_arrow_module_multi_import() {
    let src = "import sort, insert from \"collection\"\nmain = arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].path, "collection");
    assert_eq!(parsed.imports[0].names.len(), 2);
    assert!(
        parsed.imports[0]
            .names
            .contains(&("sort".to_string(), "sort".to_string()))
    );
    assert!(
        parsed.imports[0]
            .names
            .contains(&("insert".to_string(), "insert".to_string()))
    );
}

#[test]
fn test_parse_arrow_module_import_as() {
    let src = "import sort as s from \"collection/sort\"\nmain = arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(
        parsed.imports[0].names[0],
        ("sort".to_string(), "s".to_string())
    );
}

#[test]
fn test_parse_arrow_module_import_whole_module() {
    let src = "import \"collection/sort\" as sortmod\nmain = arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].path, "collection/sort");
    assert_eq!(
        parsed.imports[0].names[0],
        ("*".to_string(), "sortmod".to_string())
    );
}

#[test]
fn test_parse_arrow_module_export_names() {
    let src = "export sort, cons\nsort = arr(\"sort\")\ncons = arr(\"list.cons\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.exports.len(), 1);
    match &parsed.exports[0] {
        ExportDecl::Names(ns) => {
            assert_eq!(ns.len(), 2);
            assert!(ns.contains(&"sort".to_string()));
            assert!(ns.contains(&"cons".to_string()));
        }
        _ => panic!("expected ExportDecl::Names"),
    }
}

#[test]
fn test_parse_arrow_module_export_all() {
    let src = "export *\nsort = arr(\"sort\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.exports.len(), 1);
    assert!(matches!(parsed.exports[0], ExportDecl::All));
}

#[test]
fn test_parse_arrow_module_export_from() {
    let src = "export sort from \"collection/sort\"";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.exports.len(), 1);
    match &parsed.exports[0] {
        ExportDecl::NamesFrom(ns, path) => {
            assert_eq!(ns, &vec!["sort".to_string()]);
            assert_eq!(path, "collection/sort");
        }
        _ => panic!("expected ExportDecl::NamesFrom"),
    }
}

#[test]
fn test_parse_arrow_module_export_all_from() {
    let src = "export * from \"collection\"";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.exports.len(), 1);
    match &parsed.exports[0] {
        ExportDecl::AllFrom(path) => assert_eq!(path, "collection"),
        _ => panic!("expected ExportDecl::AllFrom"),
    }
}

#[test]
fn test_parse_arrow_module_mixed() {
    let src = "import sort from \"collection/sort\"\nimport insert from \"collection/insert\"\nexport main\nmain = arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.imports.len(), 2, "should have 2 imports");
    assert_eq!(parsed.exports.len(), 1, "should have 1 export");
}

#[test]
fn test_parse_arrow_module_no_imports_exports() {
    let src = "sort = arr(\"sort\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert!(parsed.imports.is_empty(), "should have no imports");
    assert!(parsed.exports.is_empty(), "should have no exports");
}

#[test]
fn test_parse_arrow_module_empty_after_imports() {
    let src = "export * from \"collection\"";
    let parsed = parse_arrow_module(src).expect("must parse");
    assert!(parsed.imports.is_empty());
    assert_eq!(parsed.exports.len(), 1);
    // Arrows should be empty (pure re-export module).
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => assert!(map.is_empty()),
        _ => panic!("expected empty Named map for re-export module"),
    }
}

// ---------------------------------------------------------------------------
// op/handl/cap declaration parsing tests (ADR-0077 Phase 2)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_arrow_module_op_declaration() {
    let src = r#"op "base64.encode" : String -> String
op "base64.decode" : String -> String"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.ops.len(), 2);
    assert_eq!(parsed.ops[0].name, "base64.encode");
    assert_eq!(parsed.ops[0].input_type, "String");
    assert_eq!(parsed.ops[0].output_type, "String");
    assert_eq!(parsed.ops[1].name, "base64.decode");
    assert_eq!(parsed.ops[1].input_type, "String");
    assert_eq!(parsed.ops[1].output_type, "String");
}

#[test]
fn test_parse_arrow_module_op_with_guarantee() {
    let src = r#"op "base64.encode" : String -> String
  guarantee: IsBase64(output)"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.ops.len(), 1);
    assert_eq!(parsed.ops[0].name, "base64.encode");
    assert_eq!(parsed.ops[0].guarantee.as_deref(), Some("IsBase64(output)"));
    assert!(parsed.ops[0].requires.is_none());
    assert!(parsed.ops[0].termination.is_none());
}

#[test]
fn test_parse_arrow_module_op_with_requires_and_guarantee() {
    let src = r#"op "fs.read" : Path -> Bytes
  requires: PathValid(path)
  guarantee: IsUtf8(output)"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.ops.len(), 1);
    assert_eq!(parsed.ops[0].name, "fs.read");
    assert_eq!(parsed.ops[0].input_type, "Path");
    assert_eq!(parsed.ops[0].output_type, "Bytes");
    assert_eq!(parsed.ops[0].requires.as_deref(), Some("PathValid(path)"));
    assert_eq!(parsed.ops[0].guarantee.as_deref(), Some("IsUtf8(output)"));
}

#[test]
fn test_parse_arrow_module_handl_declaration() {
    let src = r#"op "base64.encode" : String -> String
handl "base64.encode" = rust_base64_encode"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.handls.len(), 1);
    assert_eq!(parsed.handls[0].op_name, "base64.encode");
    assert_eq!(parsed.handls[0].rust_fn, "rust_base64_encode");
}

#[test]
fn test_parse_arrow_module_cap_declaration() {
    let src = r#"cap "fs.read"
cap "fs.write""#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.caps.len(), 2);
    assert_eq!(parsed.caps[0].name, "fs.read");
    assert_eq!(parsed.caps[1].name, "fs.write");
}

#[test]
fn test_parse_arrow_module_mixed_declarations() {
    let src = r#"--- base64.encode — encode a string to base64.
--- base64.decode — decode a base64 string.
op "base64.encode" : String -> String
op "base64.decode" : String -> String

handl "base64.encode" = rust_base64_encode
handl "base64.decode" = rust_base64_decode"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.ops.len(), 2);
    assert_eq!(parsed.handls.len(), 2);
    assert!(parsed.caps.is_empty());
    // Arrows should be empty (op-only module, no arrow definitions).
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => assert!(map.is_empty()),
        _ => panic!("expected empty Named map for op-only module"),
    }
}

#[test]
fn test_parse_arrow_module_op_does_not_interfere_with_arrows() {
    let src = r#"op "base64.encode" : String -> String
sort = arr("sort")"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.ops.len(), 1);
    // The arrow should still be parsed.
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => {
            assert!(map.contains_key("sort"));
        }
        _ => panic!("expected Named map with arrow definitions"),
    }
}

// ---------------------------------------------------------------------------
// tag declaration parsing tests (ADR-0077)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_arrow_module_tag_decl() {
    let src = r#"
tag ok = Value
tag err = Error
result_ok = value ~> value >>> ok
"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.tags.len(), 2);
    assert_eq!(parsed.tags[0].name, "ok");
    assert_eq!(parsed.tags[0].payload_type, "Value");
    assert_eq!(parsed.tags[1].name, "err");
    assert_eq!(parsed.tags[1].payload_type, "Error");
}

#[test]
fn test_parse_arrow_module_tag_generates_constructor_arrows() {
    let src = r#"
tag ok = Value
tag err = Error
result_ok = value ~> value >>> ok
"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    // The tag declarations should generate constructor arrows named
    // "ok" and "err" in the Named map.
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => {
            assert!(map.contains_key("ok"), "ok constructor should be generated");
            assert!(
                map.contains_key("err"),
                "err constructor should be generated"
            );
            assert!(
                map.contains_key("result_ok"),
                "result_ok arrow should be parsed"
            );
        }
        _ => panic!("expected Named map with tag constructors and arrows"),
    }
}

#[test]
fn test_parse_arrow_module_tag_only_module() {
    // A module with only tag declarations and no arrow body should still
    // parse, with the constructors in the Named map.
    let src = r#"tag ok = Value
tag err = Error"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.tags.len(), 2);
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => {
            assert_eq!(map.len(), 2);
            assert!(map.contains_key("ok"));
            assert!(map.contains_key("err"));
        }
        _ => panic!("expected Named map with tag constructors"),
    }
}

#[test]
fn test_parse_arrow_module_tag_does_not_interfere_with_arrows() {
    let src = r#"tag ok = Value
sort = arr("sort")"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.tags.len(), 1);
    assert_eq!(parsed.tags[0].name, "ok");
    match &parsed.arrows {
        crate::arrow_parser::ParsedFile::Named(map) => {
            assert!(map.contains_key("sort"));
            assert!(map.contains_key("ok"), "ok constructor should be generated");
        }
        _ => panic!("expected Named map with arrows and tag constructors"),
    }
}

#[test]
fn test_parse_arrow_module_tag_mixed_with_op_handl_cap() {
    let src = r#"tag ok = Value
op "tag.tag" : (String, Value) -> Value
cap "tag.tag"
handl "tag.tag" = rust_tag_tag"#;
    let parsed = parse_arrow_module(src).expect("must parse");
    assert_eq!(parsed.tags.len(), 1);
    assert_eq!(parsed.ops.len(), 1);
    assert_eq!(parsed.caps.len(), 1);
    assert_eq!(parsed.handls.len(), 1);
}

#[test]
fn test_file_resolver_resolves_hnl_file() {
    // Create a temp directory with a .hnl file.
    let dir = std::env::temp_dir().join("handl_file_resolver_test");
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("test_mod.hnl");
    std::fs::write(&file_path, "sort = arr(\"id\")").expect("must write test file");

    let resolver = FileResolver::new(dir.clone());
    match resolver.resolve("test_mod") {
        Ok(m) => {
            assert_eq!(m.source, "sort = arr(\"id\")");
            assert_eq!(m.origin, "file");
        }
        Err(e) => panic!("FileResolver should resolve test_mod: {}", e),
    }

    // Cleanup.
    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn test_file_resolver_not_found() {
    let resolver = FileResolver::new(PathBuf::from("/tmp/handl_nonexistent_dir_12345"));
    match resolver.resolve("missing_mod") {
        Ok(_) => panic!("FileResolver should return NotFound for missing"),
        Err(e) => assert!(matches!(e.kind, ResolveErrorKind::NotFound)),
    }
}

// ============================================================================
// Section 7: Import rewriting — module_ref/project op emission (ADR-0069)
// ============================================================================
//
// These tests verify that `parse_arrow_module` rewrites references to imported
// names into `project(module_ref(path), remote_name)` nodes (named imports) or
// `module_ref(path)` nodes (whole-module imports), and that local bindings
// shadow imports. They also verify end-to-end elimination via the
// normalisation strategies.

use handl_language::Value;

use crate::arrow_parser::ParsedFile;

/// Check whether a `Term` is a `project(module_ref(path), field)` node,
/// possibly wrapped in an `App { callee: ..., args: [] }` (the form produced
/// when a bare identifier is used as an arrow expression). Returns
/// `Some((path, field))` if it matches, `None` otherwise.
fn is_project_of_module_ref(scheme: &Term) -> Option<(String, String)> {
    let inner = match scheme {
        Term::App { callee, args } if args.is_empty() => callee.as_ref(),
        other => other,
    };
    if let Term::Arr {
        op,
        args,
        lit: Some(Value::String(field)),
        ..
    } = inner
    {
        if op == "project" && args.len() == 1 {
            if let Term::Arr {
                op: ref_op,
                args: ref_args,
                lit: Some(Value::String(path)),
                ..
            } = &args[0]
            {
                if ref_op == "module_ref" && ref_args.is_empty() {
                    return Some((path.clone(), field.clone()));
                }
            }
        }
    }
    None
}

/// Check whether a `Term` is a `module_ref(path)` node, possibly wrapped in
/// an `App { callee: ..., args: [] }`.
fn is_module_ref(scheme: &Term) -> Option<String> {
    let inner = match scheme {
        Term::App { callee, args } if args.is_empty() => callee.as_ref(),
        other => other,
    };
    if let Term::Arr {
        op,
        args,
        lit: Some(Value::String(path)),
        ..
    } = inner
    {
        if op == "module_ref" && args.is_empty() {
            return Some(path.clone());
        }
    }
    None
}

#[test]
fn test_rewrite_imports_named_import_produces_project_module_ref() {
    // `import sort from "collection/sort"` then `main = sort`.
    // After rewrite, `main` should be `project(module_ref("collection/sort"), "sort")`.
    let src = "import sort from \"collection/sort\"\nmain = sort";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            let (path, field) = is_project_of_module_ref(main)
                .expect("main should be project(module_ref(path), \"sort\")");
            assert_eq!(path, "collection/sort");
            assert_eq!(field, "sort");
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_named_import_as_alias() {
    // `import sort as s from "collection/sort"` then `main = s`.
    // After rewrite, `main` should be `project(module_ref("collection/sort"), "sort")`
    // (the remote name is used in the projection, the local alias is the
    // reference that gets rewritten).
    let src = "import sort as s from \"collection/sort\"\nmain = s";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            let (path, field) = is_project_of_module_ref(main)
                .expect("main should be project(module_ref(path), \"sort\")");
            assert_eq!(path, "collection/sort");
            assert_eq!(field, "sort");
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_local_shadow_not_rewritten() {
    // `import sort from "collection/sort"` but `sort` is also locally defined.
    // The local binding shadows the import — `sort` should NOT be rewritten.
    let src = "import sort from \"collection/sort\"\nsort = arr(\"id\")\nmain = sort";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            // `sort` is locally defined: it should remain arr("id"), not a
            // project/module_ref node.
            let sort = map.get("sort").expect("should have 'sort' arrow");
            assert!(
                is_project_of_module_ref(sort).is_none(),
                "locally-defined 'sort' should NOT be rewritten"
            );
            if let Term::Arr { op, .. } = sort {
                assert_eq!(op, "id");
            } else {
                panic!("sort should be Term::Arr");
            }

            // `main = sort` references the LOCAL sort (which is in local_names),
            // so it should NOT be rewritten either.
            let main = map.get("main").expect("should have 'main' arrow");
            assert!(
                is_project_of_module_ref(main).is_none(),
                "reference to locally-defined 'sort' should NOT be rewritten"
            );
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_whole_module_produces_module_ref() {
    // `import "collection/sort" as sortmod` then `main = sortmod`.
    // After rewrite, `main` should be `module_ref("collection/sort")`.
    let src = "import \"collection/sort\" as sortmod\nmain = sortmod";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            let path = is_module_ref(main).expect("main should be module_ref(\"collection/sort\")");
            assert_eq!(path, "collection/sort");
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_whole_module_field_access_chains() {
    // `import "collection/sort" as sortmod` then `main = comp(sortmod, arr("id"))`.
    // The `sortmod` reference inside the Seq is rewritten to
    // `module_ref("collection/sort")` (wrapped in App), and the surrounding
    // structure is preserved.
    let src = "import \"collection/sort\" as sortmod\nmain = comp(sortmod, arr(\"id\"))";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            // main should be Seq { App(module_ref("collection/sort")), arr("id") }
            if let Term::Seq { fst, snd } = main {
                let path =
                    is_module_ref(fst).expect("fst should be module_ref(\"collection/sort\")");
                assert_eq!(path, "collection/sort");
                if let Term::Arr { op, .. } = snd.as_ref() {
                    assert_eq!(op, "id");
                } else {
                    panic!("snd should be Term::Arr");
                }
            } else {
                panic!("main should be Term::Seq, got: {:?}", main);
            }
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_no_unresolved_apps_for_imported_names() {
    // After rewrite, imported names become project/module_ref ops, not bare
    // App refs to the imported name. The App's callee is now `project(...)`
    // (op = "project"), not `Arr { op: "sort" }`. So `has_unresolved_apps`
    // should return false when "project" and "module_ref" are known ops
    // (they are native ops, not named-arrow references).
    let src = "import sort from \"collection/sort\"\nmain = sort";
    let parsed = parse_arrow_module(src).expect("must parse");
    let known: HashSet<String> = vec![
        "main".to_string(),
        "project".to_string(),
        "module_ref".to_string(),
    ]
    .into_iter()
    .collect();
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            assert!(
                !has_unresolved_apps(main, &known),
                "after rewrite, imported names should not be unresolved App nodes"
            );
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

#[test]
fn test_rewrite_imports_single_expression_file() {
    // A single-expression file with an import: the import is rewritten in the
    // single expression. Single-expression files have no local names, so the
    // import always wins. `arr("id")` produces `Arr { op: "id" }` — a bare
    // reference to the "id" arrow — which is rewritten to
    // `project(module_ref("util"), "id")`.
    let src = "import id from \"util\"\narr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Single(scheme) => {
            let (path, field) = is_project_of_module_ref(scheme)
                .expect("arr(\"id\") should be rewritten to project(module_ref(\"util\"), \"id\")");
            assert_eq!(path, "util");
            assert_eq!(field, "id");
        }
        _ => panic!("expected ParsedFile::Single"),
    }
}

#[test]
fn test_rewrite_imports_app_callee_rewritten() {
    // `import sort from "collection/sort"` then `main = sort >>> arr("id")`.
    // The `sort` reference in the Seq's fst is an App whose callee is
    // rewritten to project(module_ref(path), "sort").
    let src = "import sort from \"collection/sort\"\nmain = sort >>> arr(\"id\")";
    let parsed = parse_arrow_module(src).expect("must parse");
    match &parsed.arrows {
        ParsedFile::Named(map) => {
            let main = map.get("main").expect("should have 'main' arrow");
            if let Term::Seq { fst, snd } = main {
                let (path, field) = is_project_of_module_ref(fst)
                    .expect("fst should be App { callee: project(module_ref(path), \"sort\") }");
                assert_eq!(path, "collection/sort");
                assert_eq!(field, "sort");
                if let Term::Arr { op, .. } = snd.as_ref() {
                    assert_eq!(op, "id");
                }
            } else {
                panic!("main should be Term::Seq, got: {:?}", main);
            }
        }
        _ => panic!("expected ParsedFile::Named"),
    }
}

// --- End-to-end: normalisation strategies eliminate module_ref/project ---

#[test]
fn test_e2e_normalize_compile_time_eliminates_module_ref() {
    // main imports sort from collection/sort; sort = arr("sort").
    // After rewrite + normalize_compile_time, no module_ref should remain.
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));
    match normalize_compile_time("main", &loader) {
        Ok(normalized) => {
            let any_ref = normalized
                .values()
                .any(|s| crate::arrow_loader::contains_module_ref(s));
            assert!(
                !any_ref,
                "Strategy A: no module_ref should remain after normalization"
            );
            // Strategy A preserves projections.
            let main = normalized.get("main").expect("should have 'main'");
            assert!(
                crate::arrow_loader::contains_project(main),
                "Strategy A: project nodes should remain in main arrow"
            );
        }
        Err(e) => panic!("normalize_compile_time(main) should succeed: {}", e),
    }
}

#[test]
fn test_e2e_flatten_program_eliminates_module_ref_and_project() {
    // main imports sort from collection/sort; sort = arr("sort").
    // After rewrite + flatten_program, no module_ref and no project should
    // remain — Strategy B flattens everything into a single graph.
    let mem = build_test_memory_resolver();
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("main", &loader) {
        Ok(flat) => {
            assert!(
                !crate::arrow_loader::contains_module_ref(&flat),
                "flatten_program: no module_ref should remain"
            );
            assert!(
                !crate::arrow_loader::contains_project(&flat),
                "flatten_program: no project should remain"
            );
        }
        Err(e) => panic!("flatten_program(main) should succeed: {}", e),
    }
}

#[test]
fn test_e2e_flatten_program_whole_module_import() {
    // Test a whole-module import end-to-end through flatten_program.
    let mut mem = MemoryResolver::new();
    mem.insert("util", "id = arr(\"id\")\nconst = arr(\"const\")");
    mem.insert("main_whole", "import \"util\" as u\nmain = u");
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("main_whole", &loader) {
        Ok(flat) => {
            assert!(
                !crate::arrow_loader::contains_module_ref(&flat),
                "flatten_program: no module_ref should remain for whole-module import"
            );
        }
        Err(e) => panic!("flatten_program(main_whole) should succeed: {}", e),
    }
}

// ============================================================================
// Section 7: Bare-name App discovery and file-stem aliasing
// ============================================================================

/// `flatten_program` should auto-discover bare-name `App` callees that are
/// not explicitly imported. kit-stdlib files (e.g. `sort.hnl`) reference
/// other arrows (e.g. `insert`) without import declarations; the loader
/// must load the referenced module and inline its graph.
#[test]
fn test_flatten_program_bare_name_app_discovery() {
    // `insert` is a single-expression file (keyed "main" by the loader).
    // `sort` references `insert` via `App { callee: arr("insert") }` with no
    // import. The fix: load_transitive discovers `insert`, loads it, and
    // aliases it under its file stem so `resolve_all_apps` can inline it.
    let mut mem = MemoryResolver::new();
    mem.insert("insert", "arr(\"cons\")");
    mem.insert("sort", "loop(comp(insert, first(id)))");
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("sort", &loader) {
        Ok(flat) => {
            // After flattening, no App nodes should remain — `insert` is
            // inlined.
            let known: HashSet<String> = HashSet::new();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "flatten_program: bare-name App callee 'insert' should be discovered and inlined, no unresolved apps"
            );
        }
        Err(e) => panic!("flatten_program(sort) should succeed: {}", e),
    }
}

/// Single-expression modules are keyed "main" by `ModuleLoader::load`, but
/// `load_transitive` should also alias them under their file stem so
/// bare-name references resolve. This test verifies the aliasing directly
/// by checking that a bare-name App to a single-expression module's stem
/// is inlined.
#[test]
fn test_flatten_program_file_stem_alias_for_single_expression() {
    // `helper` is a single-expression file. `caller` references `helper`
    // by its file stem (not "main") with no import. A bare identifier
    // parses to `App { callee: arr("helper"), args: [] }`.
    let mut mem = MemoryResolver::new();
    mem.insert("helper", "arr(\"id\")");
    mem.insert("caller", "helper");
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("caller", &loader) {
        Ok(flat) => {
            let known: HashSet<String> = HashSet::new();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "flatten_program: single-expression module should be aliased by file stem, no unresolved apps"
            );
        }
        Err(e) => panic!("flatten_program(caller) should succeed: {}", e),
    }
}

/// A graph that contains only primitive ops (bare `Arr` nodes, no `App`
/// nodes) should have zero unresolved apps — `has_unresolved_apps` only
/// flags `App` nodes, not bare `Arr` references to primitive ops.
#[test]
fn test_flatten_program_primitive_ops_no_unresolved_apps() {
    let mut mem = MemoryResolver::new();
    // A single-expression file using primitive ops (no App nodes).
    mem.insert(
        "prim_only",
        "loop(comp(arr(\"prim.compare\"), choice(arr(\"cons\"), arr(\"cons\"))))",
    );
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("prim_only", &loader) {
        Ok(flat) => {
            let known: HashSet<String> = HashSet::new();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "flatten_program: primitive ops (bare Arr, no App) should have zero unresolved apps"
            );
        }
        Err(e) => panic!("flatten_program(prim_only) should succeed: {}", e),
    }
}

/// Bare-name App callees that don't resolve to any module (e.g. primitive
/// op names that appear as App callees) should be skipped during discovery
/// without crashing. The App remains, but this is an edge case — the
/// important thing is the loader doesn't error.
#[test]
fn test_flatten_program_unresolvable_bare_name_skipped() {
    let mut mem = MemoryResolver::new();
    // `mystery` is not a module and not imported — discovery skips it.
    // A bare identifier parses to `App { callee: arr("mystery"), args: [] }`.
    mem.insert("main", "mystery");
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("main", &loader) {
        Ok(flat) => {
            // The App to "mystery" remains (unresolvable), but flatten
            // should not crash.
            let known: HashSet<String> = HashSet::new();
            assert!(
                has_unresolved_apps(&flat, &known),
                "flatten_program: unresolvable bare-name App should remain unresolved"
            );
        }
        Err(e) => panic!(
            "flatten_program(main) should succeed even with unresolvable callee: {}",
            e
        ),
    }
}

/// Transitive bare-name discovery: `a` references `b` (no import), `b`
/// references `c` (no import). All three should be loaded and inlined.
#[test]
fn test_flatten_program_transitive_bare_name_discovery() {
    let mut mem = MemoryResolver::new();
    mem.insert("c", "arr(\"id\")");
    mem.insert("b", "c");
    mem.insert("a", "b");
    let loader = ModuleLoader::new(Box::new(mem));
    match flatten_program("a", &loader) {
        Ok(flat) => {
            let known: HashSet<String> = HashSet::new();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "flatten_program: transitive bare-name discovery should inline all references"
            );
        }
        Err(e) => panic!("flatten_program(a) should succeed: {}", e),
    }
}

// ============================================================================
// Section 8: ADR-0077 Phase 6 — arrows/ as importable modules
// ============================================================================
//
// These tests verify that `handl-packages/*.hnl` files with
// `import`/`export` declarations are loadable via the `ModuleLoader`, and
// that compositional inference works across the import boundary.
//
// The real arrow file contents are embedded via `include_str!` so the tests
// have no filesystem dependency (they use `MemoryResolver`).

/// The source text of `collection/insert.hnl` (with `export insert`).
const INSERT_SRC: &str = include_str!("../../../handl-packages/collection/insert.hnl");

/// The source text of `collection/sort.hnl` (with `import insert`).
const SORT_SRC: &str = include_str!("../../../handl-packages/collection/sort.hnl");

/// Build a `MemoryResolver` populated with the real arrow file contents,
/// rooted at the arrows directory layout (e.g. `"collection/sort"` →
/// `sort.hnl` content).
fn build_real_arrows_resolver() -> MemoryResolver {
    let mut m = MemoryResolver::new();
    m.insert("collection/insert", INSERT_SRC);
    m.insert("collection/sort", SORT_SRC);
    m
}

/// (a) Basic import: Load `collection/sort.hnl` via the `ModuleLoader` with
/// a `MemoryResolver` populated with the real arrow file contents. Verify
/// the ModuleLoader resolves the `import insert from "collection/insert"`
/// and `flatten_program` produces a `Term` for `sort` that includes
/// `insert`'s graph (no unresolved `module_ref`, `project`, or `App` nodes
/// to unknown names).
#[test]
fn test_phase6_basic_import() {
    let mem = build_real_arrows_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    // Load sort — should succeed and parse the import declaration.
    let _loaded = loader
        .load("collection/sort")
        .expect("load collection/sort");
    // The import should be recorded.
    let imports = loader.get_imports("collection/sort").expect("get imports");
    assert!(
        imports.iter().any(|i| i.path == "collection/insert"),
        "sort should import from collection/insert: {:?}",
        imports
    );

    // flatten_program should inline insert's graph into sort's graph.
    let flat = flatten_program("collection/sort", &loader).expect("flatten sort");
    // No module_ref or project nodes should remain after flattening.
    assert!(
        !crate::arrow_loader::contains_module_ref(&flat),
        "flatten_program: no module_ref should remain after inlining insert"
    );
    assert!(
        !crate::arrow_loader::contains_project(&flat),
        "flatten_program: no project should remain after inlining insert"
    );
    // No unresolved App nodes — insert should be fully inlined.
    let known: HashSet<String> = HashSet::new();
    assert!(
        !has_unresolved_apps(&flat, &known),
        "flatten_program: insert should be inlined, no unresolved App nodes"
    );
}

/// (b) Compositional inference: Load `collection/sort.hnl`, flatten it
/// (which inlines `insert`'s graph across the import boundary), and infer
/// its contract. Verify the `Sorted(output)` contract is inferred —
/// composing across the import boundary from `insert`'s contract.
#[test]
fn test_phase6_compositional_inference() {
    let mem = build_real_arrows_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    // Flatten sort — inlines insert's graph via the import boundary.
    let flat = flatten_program("collection/sort", &loader).expect("flatten sort");

    // Infer the contract on the flattened graph.
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&flat, &registry).expect("infer sort (flattened)");
    let pretty = result.guarantee.pretty();

    // The Sorted contract should be inferred, composing insert's
    // Sorted guarantee across the import boundary.
    assert!(
        pretty.contains("Sorted"),
        "sort (flattened with import) should infer Sorted: {}",
        pretty
    );
}

/// (c) Full cascade: Verify inference composes across the full cascade
/// `sort` → `insert` → `prim.compare`. The `insert` arrow's graph contains
/// `prim.compare` as a primitive op; when `sort` is flattened with
/// `insert` inlined, the inference engine sees the full graph including
/// `prim.compare` and infers `Sorted`.
#[test]
fn test_phase6_full_cascade() {
    let mem = build_real_arrows_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    // Load insert directly and verify it infers Sorted.
    let insert_flat = flatten_program("collection/insert", &loader).expect("flatten insert");
    let registry = ContractRegistry::minimal();
    let insert_result = infer_refinement(&insert_flat, &registry).expect("infer insert");
    let insert_pretty = insert_result.guarantee.pretty();
    assert!(
        insert_pretty.contains("Sorted"),
        "insert should infer Sorted: {}",
        insert_pretty
    );

    // Load sort (which imports insert) and verify the full cascade
    // composes: sort → insert → prim.compare → Sorted.
    let sort_flat = flatten_program("collection/sort", &loader).expect("flatten sort");
    let sort_result = infer_refinement(&sort_flat, &registry).expect("infer sort");
    let sort_pretty = sort_result.guarantee.pretty();
    assert!(
        sort_pretty.contains("Sorted"),
        "sort full cascade (sort → insert → prim.compare) should infer Sorted: {}",
        sort_pretty
    );
}

/// Verify that the `export insert` declaration in `insert.hnl` is correctly
/// parsed and the ModuleLoader reports `insert` as a public name.
#[test]
fn test_phase6_export_declaration() {
    let mem = build_real_arrows_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    // insert.hnl has `export insert` — public_names should include "insert".
    let names = loader
        .public_names("collection/insert")
        .expect("public names");
    assert!(
        names.contains(&"insert".to_string()),
        "insert.hnl should export 'insert': {:?}",
        names
    );
}

/// Verify that `normalize_compile_time` (Strategy A) also handles the
/// import boundary — the normalized sort arrow should have no
/// `module_ref` nodes (they are replaced by inlined records).
#[test]
fn test_phase6_normalize_compile_time() {
    let mem = build_real_arrows_resolver();
    let loader = ModuleLoader::new(Box::new(mem));

    match normalize_compile_time("collection/sort", &loader) {
        Ok(normalized) => {
            let any_ref = normalized
                .values()
                .any(|s| crate::arrow_loader::contains_module_ref(s));
            assert!(
                !any_ref,
                "Strategy A: no module_ref should remain after normalization"
            );
        }
        Err(e) => panic!(
            "normalize_compile_time(collection/sort) should succeed: {}",
            e
        ),
    }
}
