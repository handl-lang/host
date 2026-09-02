//! Contract registry — the separate registry for base-arrow contracts.
//!
//! Per ADR-0052, contracts do NOT go in `Scheme`. They live in a separate
//! registry keyed by `(op, source)`, mirroring the research prototype's
//! `RefSigEntry` table but adapted to handl's `ArrSource` discipline.
//!
//! Two registries are provided:
//! - [`ContractRegistry::minimal`] — the reduced 3-atom registry modelling
//!   only the irreducible Rust primitives (`prim.add`, `prim.compare`,
//!   `prim.alloc`). This is the minimal setup used by the extended
//!   refinement plugin for Choice WP branching and conditional contracts.
//! - [`ContractRegistry::standard`] — the legacy 18-contract registry ported
//!   from the research prototype. Retained for backward compatibility with
//!   the existing test suite; new code should prefer `minimal()`.

use std::collections::HashMap;

use crate::types::{BaseTy, Contract, Pred, RefTy};
use handl_language::ArrSource;

/// Convert an `ArrSource` to a stable string key for use in a `HashMap`.
///
/// `ArrSource` does not derive `Hash`/`Eq` (it's in `handl-language` which we
/// cannot modify), so we use a string representation as the key.
fn source_key(source: &ArrSource) -> String {
    match source {
        ArrSource::Native => "Native".to_string(),
        ArrSource::Foreign(name) => format!("Foreign({})", name),
        ArrSource::Judgment => "Judgment".to_string(),
        ArrSource::DynamicForeign => "DynamicForeign".to_string(),
    }
}

/// A registry of contracts for base arrows, keyed by `(op, source)`.
///
/// This is the handl-side equivalent of the research prototype's
/// `Vec<RefSigEntry>` signature table. Contracts are looked up by the
/// inference engine when it encounters a `Scheme::Arr` node.
#[derive(Debug)]
pub struct ContractRegistry {
    contracts: HashMap<(String, String), Contract>,
}

impl ContractRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
        }
    }

    /// Register a contract for a given `(op, source)` pair.
    pub fn register(&mut self, op: impl Into<String>, source: ArrSource, contract: Contract) {
        self.contracts
            .insert((op.into(), source_key(&source)), contract);
    }

    /// Look up the contract for a given `(op, source)` pair.
    pub fn lookup(&self, op: &str, source: &ArrSource) -> Option<&Contract> {
        self.contracts
            .get(&(op.to_string(), source_key(source)))
    }

    /// Returns `true` if a contract is registered for the given `(op, source)`.
    pub fn contains(&self, op: &str, source: &ArrSource) -> bool {
        self.contracts
            .contains_key(&(op.to_string(), source_key(source)))
    }

    /// Returns the number of contracts registered.
    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    /// Returns `true` if the registry holds no contracts.
    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    /// Merge another registry into this one. Contracts in `other` overwrite
    /// any existing entry with the same `(op, source)` key.
    pub fn merge(&mut self, other: ContractRegistry) {
        for (k, v) in other.contracts {
            self.contracts.insert(k, v);
        }
    }

    /// The minimal set of contracts: only the three irreducible Rust atoms.
    ///
    /// This is the reduced registry for the minimal 3-atom setup. It models
    /// the primitive operations that cannot be decomposed further:
    ///
    /// - `prim.add`     : `(Int × Int) → Int` — integer addition.
    /// - `prim.compare` : `(α × β) → Int`    — comparison, yields a boolean-ish
    ///   result (treated as a comparison result).
    /// - `prim.alloc`   : `Int → List α`     — allocation, yields an empty list.
    ///
    /// All three carry an unconditional (`True`) precondition and a structural
    /// guarantee about their output. Higher-level operations (sort, insert,
    /// map, filter, …) are built compositionally from these atoms rather than
    /// being registered directly.
    pub fn minimal() -> Self {
        let mut reg = Self::new();

        let alpha = || RefTy::Plain(BaseTy::Var("α".to_string()));
        let beta = || RefTy::Plain(BaseTy::Var("β".to_string()));
        let int = || RefTy::Plain(BaseTy::Int);
        let prod_int_int = || RefTy::Prod(Box::new(int()), Box::new(int()));
        let prod_ab = || RefTy::Prod(Box::new(alpha()), Box::new(beta()));

        // prim.add : (Int × Int) → Int — integer addition.
        // Guarantee: the output is an integer (trivially true for the base
        // type, but recorded so the inference engine can reason about it).
        reg.register(
            "prim.add",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // prim.compare : (α × β) → Int — comparison.
        // Returns -1/0/1. Guarantee: the output is ≤ 1 (a comparison result).
        reg.register(
            "prim.compare",
            ArrSource::Native,
            Contract {
                input: prod_ab(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::Le("output".to_string(), "1".to_string()),
                termination_effect: None,
            },
        );

        // prim.alloc : Int → List α — allocation.
        // Allocates a fresh empty list of the requested capacity.
        // Guarantee: the output list has length 0.
        reg.register(
            "prim.alloc",
            ArrSource::Native,
            Contract {
                input: int(),
                output: RefTy::List(Box::new(alpha())),
                precondition: Pred::True,
                guarantee: Pred::LengthEq("output".to_string(), "0".to_string()),
                termination_effect: None,
            },
        );

        // prim.sub : (Int × Int) → Int — integer subtraction.
        reg.register(
            "prim.sub",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // prim.mul : (Int × Int) → Int — integer multiplication.
        reg.register(
            "prim.mul",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // prim.le : (Int × Int) → Bool — less-than-or-equal comparison.
        reg.register(
            "prim.le",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // prim.eq : (α × α) → Bool — equality test (polymorphic).
        reg.register(
            "prim.eq",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(alpha()), Box::new(alpha())),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // list.cons : (α × List α) → List α — prepend element to list.
        // Conditional contract: if the tail is sorted AND the head is ≤ the
        // first element of the tail, then the output is sorted. The output
        // length is always one more than the tail's length.
        let list_alpha = || RefTy::List(Box::new(alpha()));
        reg.register(
            "list.cons",
            ArrSource::Native,
            Contract::conditional(
                RefTy::Prod(Box::new(alpha()), Box::new(list_alpha())),
                list_alpha(),
                Pred::And(
                    Box::new(Pred::Sorted("tail".to_string())),
                    Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
                ),
                Pred::And(
                    Box::new(Pred::And(
                        Box::new(Pred::Sorted("output".to_string())),
                        Box::new(Pred::LengthPlusOne("output".to_string(), "tail".to_string())),
                    )),
                    Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
                ),
                None,
            ),
        );

        // list.head : List α → α — get first element.
        reg.register(
            "list.head",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // list.tail : List α → List α — get rest of list.
        // Conditional: if the input is sorted, the tail is sorted and one
        // element shorter.
        reg.register(
            "list.tail",
            ArrSource::Native,
            Contract::conditional(
                list_alpha(),
                list_alpha(),
                Pred::Sorted("input".to_string()),
                Pred::And(
                    Box::new(Pred::And(
                        Box::new(Pred::Sorted("output".to_string())),
                        Box::new(Pred::LengthSubOne("output".to_string(), "input".to_string())),
                    )),
                    Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
                ),
                None,
            ),
        );

        // list.is_empty : List α → Bool — test if list is empty.
        reg.register(
            "list.is_empty",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // list.length : List α → Int — list length.
        reg.register(
            "list.length",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // list.append : (List α × List α) → List α — list concatenation.
        // NOTE: the ideal guarantee is LengthEq(output, input.0 + input.1),
        // but the contract-file parser does not yet support arithmetic
        // expressions in predicate arguments, so we record True here.
        reg.register(
            "list.append",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(list_alpha()), Box::new(list_alpha())),
                output: list_alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // list.reverse : List α → List α — list reversal.
        // Guarantee: the output has the same length as the input.
        reg.register(
            "list.reverse",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: list_alpha(),
                precondition: Pred::True,
                guarantee: Pred::LengthEq("output".to_string(), "input".to_string()),
                termination_effect: None,
            },
        );

        // list.nth : (List α × Int) → α — element at index.
        reg.register(
            "list.nth",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(list_alpha()), Box::new(int())),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // id : α → α — identity (used as pass-through in loops).
        reg.register(
            "id",
            ArrSource::Native,
            Contract {
                input: alpha(),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // cons : (α × List α) → List α — alias for list.cons (used in arrow graphs).
        // Same conditional contract as list.cons.
        reg.register(
            "cons",
            ArrSource::Native,
            Contract::conditional(
                RefTy::Prod(Box::new(alpha()), Box::new(list_alpha())),
                list_alpha(),
                Pred::And(
                    Box::new(Pred::Sorted("tail".to_string())),
                    Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
                ),
                Pred::And(
                    Box::new(Pred::And(
                        Box::new(Pred::Sorted("output".to_string())),
                        Box::new(Pred::LengthPlusOne("output".to_string(), "tail".to_string())),
                    )),
                    Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
                ),
                None,
            ),
        );

        // fn.apply : (α → β) × α → β — function application (used by map).
        // Treated as an opaque atom; its guarantee depends on the applied function.
        reg.register(
            "fn.apply",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(alpha()), Box::new(alpha())),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // filter_step : List α → List α — one step of filtering (used by filter).
        // Guarantee: the output satisfies the predicate and is no longer than
        // the input.
        reg.register(
            "filter_step",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: list_alpha(),
                precondition: Pred::True,
                guarantee: Pred::And(
                    Box::new(Pred::AllSatisfy("output".to_string(), "predicate".to_string())),
                    Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
                ),
                termination_effect: None,
            },
        );

        // fold_step : (α × β) → α — one step of folding (used by fold).
        // Opaque atom; guarantee depends on the step function. However, the
        // *termination* behaviour is structural: each fold step consumes one
        // element from the input list (the feedback channel C), so |C|
        // decreases by 1 each iteration. This lets the termination analyser
        // prove that `loop(comp(fold_step, first(id)))` terminates with
        // measure ListLength.
        reg.register(
            "fold_step",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(alpha()), Box::new(alpha())),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("decrements_length".to_string()),
            },
        );

        // ==================================================================
        // Parse atoms — token-based parsing primitives (Pratt parser).
        //
        // These model the primitive operations of a Pratt parser (top-down
        // operator-precedence parser). The key atom is parse.consume, which
        // removes one token from the token list — its LengthSubOne guarantee
        // and decrements_length termination effect let the inference engine
        // derive that the parser consumes tokens and terminates.
        // ==================================================================

        let token_list = || RefTy::Plain(BaseTy::Var("TokenList".to_string()));
        let token = || RefTy::Plain(BaseTy::Var("Token".to_string()));
        let ast = || RefTy::Plain(BaseTy::Var("AST".to_string()));

        // parse.peek : TokenList → Token — look at the next token without consuming.
        reg.register(
            "parse.peek",
            ArrSource::Native,
            Contract {
                input: token_list(),
                output: token(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // parse.consume : TokenList → TokenList — consume one token.
        // Guarantee: the output token list is one element shorter than the input.
        // Termination: each consume decrements the token list length by 1.
        reg.register(
            "parse.consume",
            ArrSource::Native,
            Contract {
                input: token_list(),
                output: token_list(),
                precondition: Pred::True,
                guarantee: Pred::LengthSubOne("output".to_string(), "input".to_string()),
                termination_effect: Some("decrements_length".to_string()),
            },
        );

        // parse.is_op : Token → Bool — test whether a token is an operator.
        reg.register(
            "parse.is_op",
            ArrSource::Native,
            Contract {
                input: token(),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // parse.prec : Token → Int — get the precedence of an operator token.
        reg.register(
            "parse.prec",
            ArrSource::Native,
            Contract {
                input: token(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // parse.make_node : (Token × AST × AST) → AST — build an AST node
        // from an operator and two sub-expressions.
        reg.register(
            "parse.make_node",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(
                    Box::new(token()),
                    Box::new(RefTy::Prod(Box::new(ast()), Box::new(ast()))),
                ),
                output: ast(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // parse.atom_value : Token → AST — convert a token to an AST leaf node.
        reg.register(
            "parse.atom_value",
            ArrSource::Native,
            Contract {
                input: token(),
                output: ast(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: None,
            },
        );

        // ==================================================================
        // State machine atoms — transitions for counters, buffers, and
        // protocol state machines.
        //
        // These model the primitive operations of state machines. The key
        // atoms are state.inc/state.dec (counter) and state.push/state.pop
        // (buffer). state.dec has a conditional contract with Le(0, input)
        // precondition and Le(0, output) guarantee, so the fixpoint can
        // discover Count ≥ 0 as the loop invariant.
        // ==================================================================

        let state_ty = || RefTy::Plain(BaseTy::Var("State".to_string()));

        // state.inc : Int → Int — increment a counter.
        // Guarantee: output ≥ input (Count increases by 1).
        reg.register(
            "state.inc",
            ArrSource::Native,
            Contract {
                input: int(),
                output: int(),
                precondition: Pred::True,
                guarantee: Pred::Le("input".to_string(), "output".to_string()),
                termination_effect: Some("constant".to_string()),
            },
        );

        // state.dec : Int → Int — decrement a counter (only valid if Count > 0).
        // Conditional contract: requires Le(0, input) (Count ≥ 0),
        // guarantees Le(0, output) (Count stays ≥ 0 after decrement).
        // This lets the fixpoint discover Le(0, count) as the invariant.
        reg.register(
            "state.dec",
            ArrSource::Native,
            Contract::conditional(
                int(),
                int(),
                Pred::Le("0".to_string(), "input".to_string()),
                Pred::Le("0".to_string(), "output".to_string()),
                Some("constant".to_string()),
            ),
        );

        // state.push : (List α × α) → List α — add item to buffer.
        // Guarantee: output is one element longer than input.
        reg.register(
            "state.push",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(Box::new(list_alpha()), Box::new(alpha())),
                output: list_alpha(),
                precondition: Pred::True,
                guarantee: Pred::LengthPlusOne("output".to_string(), "input".to_string()),
                termination_effect: Some("constant".to_string()),
            },
        );

        // state.pop : List α → List α — remove item from buffer.
        // Conditional contract: requires Le(0, input) (buffer non-empty),
        // guarantees LengthSubOne(output, input) ∧ Le(0, output)
        // (buffer shrinks by 1 and stays non-negative).
        reg.register(
            "state.pop",
            ArrSource::Native,
            Contract::conditional(
                list_alpha(),
                list_alpha(),
                Pred::Le("0".to_string(), "input".to_string()),
                Pred::And(
                    Box::new(Pred::LengthSubOne("output".to_string(), "input".to_string())),
                    Box::new(Pred::Le("0".to_string(), "output".to_string())),
                ),
                Some("constant".to_string()),
            ),
        );

        // state.is_empty : List α → Bool — check if buffer is empty.
        reg.register(
            "state.is_empty",
            ArrSource::Native,
            Contract {
                input: list_alpha(),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("constant".to_string()),
            },
        );

        // state.idle_to_active : State → State — protocol transition.
        reg.register(
            "state.idle_to_active",
            ArrSource::Native,
            Contract {
                input: state_ty(),
                output: state_ty(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("constant".to_string()),
            },
        );

        // state.active_to_done : State → State — protocol transition.
        reg.register(
            "state.active_to_done",
            ArrSource::Native,
            Contract {
                input: state_ty(),
                output: state_ty(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("constant".to_string()),
            },
        );

        reg
    }

    /// Load the canonical axiom set from the bundled handl contract files.
    ///
    /// This is the new canonical way to obtain the minimal registry: it reads
    /// the `.hnl` contract files bundled with the crate (via `include_str!`)
    /// and parses them with the loader. The result is equivalent to
    /// [`ContractRegistry::minimal`] for inference purposes.
    ///
    /// `minimal()` and `standard()` are retained for backward compatibility,
    /// but new code should prefer `from_axioms()`.
    pub fn from_axioms() -> Self {
        crate::loader::load_standard_axioms()
    }

    /// The standard set of contracts, ported from the research prototype's
    /// `real_programs_sig()` and `standard_sig()`.
    ///
    /// **Legacy / deprecated:** this is the old 18-contract registry retained
    /// for backward compatibility with the existing test suite. New code
    /// should prefer [`ContractRegistry::minimal`], which models only the
    /// irreducible Rust atoms and lets higher-level operations be derived
    /// compositionally.
    ///
    /// These model the primitive operations the algorithms are built from.
    /// Each carries a known refinement guarantee (its type signature, not a
    /// user annotation) and a termination effect on the feedback channel C
    /// when used as the right-hand side of a loop body.
    pub fn standard() -> Self {
        let mut reg = Self::new();

        let alpha = || RefTy::Plain(BaseTy::Var("α".to_string()));
        let beta = || RefTy::Plain(BaseTy::Var("β".to_string()));
        let list_a = || RefTy::List(Box::new(alpha()));
        let list_b = || RefTy::List(Box::new(beta()));
        let prod_aa = || RefTy::Prod(Box::new(alpha()), Box::new(list_a()));
        let prod_int_int = || RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Int)),
            Box::new(RefTy::Plain(BaseTy::Int)),
        );

        // head_and_tail : List α → (α × List α) — split list into head and tail
        reg.register(
            "head_and_tail",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: prod_aa(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // insert : (α × List α) → List α — insert element into sorted list
        // Guarantee: output is sorted.
        reg.register(
            "insert",
            ArrSource::Native,
            Contract {
                input: prod_aa(),
                output: RefTy::Refine(
                    BaseTy::Var("α".to_string()),
                    Pred::Sorted("output".to_string()),
                ),
                precondition: Pred::True,
                guarantee: Pred::Sorted("output".to_string()),
                termination_effect: Some("none".to_string()),
            },
        );

        // append : (α × List α) → List α — append element to list
        reg.register(
            "append",
            ArrSource::Native,
            Contract {
                input: prod_aa(),
                output: list_a(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // apply_f : α → β — apply function f
        reg.register(
            "apply_f",
            ArrSource::Native,
            Contract {
                input: alpha(),
                output: beta(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // test_predicate : α → Bool — test predicate
        reg.register(
            "test_predicate",
            ArrSource::Native,
            Contract {
                input: alpha(),
                output: RefTy::Plain(BaseTy::Bool),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // add_to_acc : (Int × Int) → Int — add two integers
        reg.register(
            "add_to_acc",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: RefTy::Plain(BaseTy::Int),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // consume_one : List α → List α — remove one element (termination effect)
        reg.register(
            "consume_one",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: list_a(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("decrements_length".to_string()),
            },
        );

        // remove_one : List α → List α — removes one element (sort helper)
        reg.register(
            "remove_one",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: list_a(),
                precondition: Pred::True,
                guarantee: Pred::LengthLe("output".to_string(), "input".to_string()),
                termination_effect: Some("decrements_length".to_string()),
            },
        );

        // sort : List α → List α — sort, guarantee: Sorted
        reg.register(
            "sort",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: RefTy::Refine(
                    BaseTy::Var("α".to_string()),
                    Pred::Sorted("output".to_string()),
                ),
                precondition: Pred::True,
                guarantee: Pred::Sorted("output".to_string()),
                termination_effect: Some("none".to_string()),
            },
        );

        // id : α → α — identity
        reg.register(
            "id",
            ArrSource::Native,
            Contract {
                input: alpha(),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("constant".to_string()),
            },
        );

        // map : List α → List β — length-preserving map
        reg.register(
            "map",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: list_b(),
                precondition: Pred::True,
                guarantee: Pred::LengthEq("input".to_string(), "output".to_string()),
                termination_effect: Some("none".to_string()),
            },
        );

        // map_step : List α → List β — one map step (length-preserving)
        reg.register(
            "map_step",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: list_b(),
                precondition: Pred::True,
                guarantee: Pred::LengthEq("input".to_string(), "output".to_string()),
                termination_effect: Some("none".to_string()),
            },
        );

        // filter : List α → List α — filter, guarantee: AllSatisfy ∧ LengthLe
        reg.register(
            "filter",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: RefTy::Refine(
                    BaseTy::Var("α".to_string()),
                    Pred::AllSatisfy("output".to_string(), "predicate".to_string()),
                ),
                precondition: Pred::True,
                guarantee: Pred::And(
                    Box::new(Pred::AllSatisfy(
                        "output".to_string(),
                        "predicate".to_string(),
                    )),
                    Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
                ),
                termination_effect: Some("none".to_string()),
            },
        );

        // filter_step : List α → List α — one filter step
        reg.register(
            "filter_step",
            ArrSource::Native,
            Contract {
                input: list_a(),
                output: RefTy::Refine(
                    BaseTy::Var("α".to_string()),
                    Pred::AllSatisfy("output".to_string(), "predicate".to_string()),
                ),
                precondition: Pred::True,
                guarantee: Pred::And(
                    Box::new(Pred::AllSatisfy(
                        "output".to_string(),
                        "predicate".to_string(),
                    )),
                    Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
                ),
                termination_effect: Some("none".to_string()),
            },
        );

        // append_to_output : (α × List α) → List α — append to output accumulator
        reg.register(
            "append_to_output",
            ArrSource::Native,
            Contract {
                input: prod_aa(),
                output: list_a(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // decrement : Int → Int — decrement counter by 1
        reg.register(
            "decrement",
            ArrSource::Native,
            Contract {
                input: RefTy::Plain(BaseTy::Int),
                output: RefTy::Plain(BaseTy::Int),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("decrements_counter".to_string()),
            },
        );

        // step : (α × Int) → (β × Int) — step function (does not touch C's measure)
        reg.register(
            "step",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(
                    Box::new(alpha()),
                    Box::new(RefTy::Plain(BaseTy::Int)),
                ),
                output: RefTy::Prod(
                    Box::new(beta()),
                    Box::new(RefTy::Plain(BaseTy::Int)),
                ),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // process_one : (α × List β) → (γ × List β) — process one element
        reg.register(
            "process_one",
            ArrSource::Native,
            Contract {
                input: RefTy::Prod(
                    Box::new(alpha()),
                    Box::new(RefTy::List(Box::new(beta()))),
                ),
                output: RefTy::Prod(
                    Box::new(RefTy::Plain(BaseTy::Var("γ".to_string()))),
                    Box::new(RefTy::List(Box::new(beta()))),
                ),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        // inner_terminating : (Int × Int) → (Int × Int) — inner loop body that terminates
        reg.register(
            "inner_terminating",
            ArrSource::Native,
            Contract {
                input: prod_int_int(),
                output: prod_int_int(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("decrements_counter".to_string()),
            },
        );

        // f : α → α — generic function (no decrease, for fixed-point loops)
        reg.register(
            "f",
            ArrSource::Native,
            Contract {
                input: alpha(),
                output: alpha(),
                precondition: Pred::True,
                guarantee: Pred::True,
                termination_effect: Some("none".to_string()),
            },
        );

        reg
    }
}

impl Default for ContractRegistry {
    fn default() -> Self {
        Self::new()
    }
}
