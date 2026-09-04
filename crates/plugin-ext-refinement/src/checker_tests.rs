//! Tests for the runtime axiom checker bridge (ADR-0077 Phase 3).
//!
//! These tests verify:
//! - [`Pred::eval`] evaluates predicates against runtime JSON values correctly.
//! - [`ContractChecker`] enforces `requires`/`guarantee` predicates.
//! - [`register_contract_checkers`] wires checkers into an [`OpRegistry`]
//!   and they fire via [`OpFunction::call`].

use std::sync::Arc;

use handl_language::ArrSource;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{OpFunction, OpRegistry, RuntimePredicateChecker};
use serde_json::{json, Value};

use crate::checker::{register_contract_checkers, ContractChecker};
use crate::contracts::ContractRegistry;
use crate::types::{BaseTy, Contract, Pred, RefTy};

// ============================================================================
// Pred::eval tests
// ============================================================================

#[test]
fn pred_eval_true_false() {
    assert!(Pred::True.eval(&json!(42)));
    assert!(Pred::True.eval(&json!("anything")));
    assert!(Pred::True.eval(&json!(null)));
    assert!(!Pred::False.eval(&json!(42)));
    assert!(!Pred::False.eval(&json!("anything")));
}

#[test]
fn pred_eval_sorted() {
    // Sorted ascending → true.
    assert!(Pred::Sorted("x".to_string()).eval(&json!([1, 2, 3])));
    // Empty array is vacuously sorted.
    assert!(Pred::Sorted("x".to_string()).eval(&json!([])));
    // Non-decreasing (duplicates ok).
    assert!(Pred::Sorted("x".to_string()).eval(&json!([1, 1, 2, 2])));
    // Unsorted → false.
    assert!(!Pred::Sorted("x".to_string()).eval(&json!([3, 1, 2])));
    // Non-array → false (fail-closed).
    assert!(!Pred::Sorted("x".to_string()).eval(&json!(42)));
    assert!(!Pred::Sorted("x".to_string()).eval(&json!("hello")));
    // Array with non-number → false.
    assert!(!Pred::Sorted("x".to_string()).eval(&json!([1, "two", 3])));
}

#[test]
fn pred_eval_le() {
    // Object lookup: a <= b.
    assert!(Pred::Le("a".to_string(), "b".to_string()).eval(&json!({"a": 1, "b": 2})));
    assert!(Pred::Le("a".to_string(), "b".to_string()).eval(&json!({"a": 2, "b": 2})));
    assert!(!Pred::Le("a".to_string(), "b".to_string()).eval(&json!({"a": 3, "b": 2})));
    // Array index lookup: index 0 <= index 1.
    assert!(Pred::Le("0".to_string(), "1".to_string()).eval(&json!([1, 5])));
    assert!(!Pred::Le("0".to_string(), "1".to_string()).eval(&json!([5, 1])));
    // Non-number → false.
    assert!(!Pred::Le("a".to_string(), "b".to_string()).eval(&json!({"a": "x", "b": 2})));
}

#[test]
fn pred_eval_and_or_implies() {
    let t = Pred::True;
    let f = Pred::False;

    // And.
    assert!(Pred::And(Box::new(t.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(!Pred::And(Box::new(t.clone()), Box::new(f.clone())).eval(&json!(null)));
    assert!(!Pred::And(Box::new(f.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(!Pred::And(Box::new(f.clone()), Box::new(f.clone())).eval(&json!(null)));

    // Or.
    assert!(Pred::Or(Box::new(t.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(Pred::Or(Box::new(t.clone()), Box::new(f.clone())).eval(&json!(null)));
    assert!(Pred::Or(Box::new(f.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(!Pred::Or(Box::new(f.clone()), Box::new(f.clone())).eval(&json!(null)));

    // Implies: True ==> True → true, True ==> False → false, False ==> _ → true.
    assert!(Pred::Implies(Box::new(t.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(!Pred::Implies(Box::new(t.clone()), Box::new(f.clone())).eval(&json!(null)));
    assert!(Pred::Implies(Box::new(f.clone()), Box::new(t.clone())).eval(&json!(null)));
    assert!(Pred::Implies(Box::new(f.clone()), Box::new(f.clone())).eval(&json!(null)));
}

#[test]
fn pred_eval_length_eq() {
    // Object with two array fields.
    assert!(Pred::LengthEq("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [3, 4]})));
    assert!(!Pred::LengthEq("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [3]})));
    // Non-array → false.
    assert!(!Pred::LengthEq("a".to_string(), "b".to_string())
        .eval(&json!({"a": 1, "b": 2})));
}

#[test]
fn pred_eval_length_le() {
    assert!(Pred::LengthLe("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1], "b": [3, 4]})));
    assert!(Pred::LengthLe("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [3, 4]})));
    assert!(!Pred::LengthLe("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2, 3], "b": [3, 4]})));
}

#[test]
fn pred_eval_length_plus_one_sub_one() {
    // LengthPlusOne: |a| = |b| + 1.
    assert!(Pred::LengthPlusOne("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2, 3], "b": [4, 5]})));
    assert!(!Pred::LengthPlusOne("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [4, 5]})));
    // LengthSubOne: |a| = |b| - 1 (i.e. |a| + 1 = |b|).
    assert!(Pred::LengthSubOne("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [4, 5, 6]})));
    assert!(!Pred::LengthSubOne("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [4, 5]})));
}

#[test]
fn pred_eval_is_perm() {
    assert!(Pred::IsPerm("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2, 3], "b": [3, 1, 2]})));
    assert!(!Pred::IsPerm("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2, 3], "b": [1, 2, 4]})));
    assert!(!Pred::IsPerm("a".to_string(), "b".to_string())
        .eval(&json!({"a": [1, 2], "b": [1, 2, 3]})));
}

#[test]
fn pred_eval_head_of() {
    // elem is the first element of list.
    assert!(Pred::HeadOf("elem".to_string(), "list".to_string())
        .eval(&json!({"elem": 1, "list": [1, 2, 3]})));
    assert!(!Pred::HeadOf("elem".to_string(), "list".to_string())
        .eval(&json!({"elem": 2, "list": [1, 2, 3]})));
    // Empty list → false.
    assert!(!Pred::HeadOf("elem".to_string(), "list".to_string())
        .eval(&json!({"elem": 1, "list": []})));
}

#[test]
fn pred_eval_is_success_is_error() {
    // __choice__ == "left" → success.
    assert!(Pred::IsSuccess("x".to_string()).eval(&json!({"__choice__": "left", "value": 42})));
    assert!(!Pred::IsSuccess("x".to_string()).eval(&json!({"__choice__": "right", "value": 42})));
    // "ok" field → success (Result-like convention).
    assert!(Pred::IsSuccess("x".to_string()).eval(&json!({"ok": 42})));
    // __choice__ == "right" → error.
    assert!(Pred::IsError("x".to_string()).eval(&json!({"__choice__": "right", "value": "err"})));
    assert!(!Pred::IsError("x".to_string()).eval(&json!({"__choice__": "left", "value": 42})));
    // "err"/"error" field → error.
    assert!(Pred::IsError("x".to_string()).eval(&json!({"err": "boom"})));
    assert!(Pred::IsError("x".to_string()).eval(&json!({"error": "boom"})));
    // Non-object → false.
    assert!(!Pred::IsSuccess("x".to_string()).eval(&json!(42)));
    assert!(!Pred::IsError("x".to_string()).eval(&json!(42)));
}

#[test]
fn pred_eval_all_satisfy() {
    // All elements satisfy "Sorted" (each element is a sorted sub-array).
    assert!(Pred::AllSatisfy("list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": [[1, 2], [3, 4]]})));
    assert!(!Pred::AllSatisfy("list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": [[1, 2], [4, 3]]})));
    // "True" predicate → always holds.
    assert!(Pred::AllSatisfy("list".to_string(), "True".to_string())
        .eval(&json!({"list": [1, 2, 3]})));
    // "False" predicate → never holds (unless empty).
    assert!(!Pred::AllSatisfy("list".to_string(), "False".to_string())
        .eval(&json!({"list": [1, 2, 3]})));
    // Empty list → vacuously true.
    assert!(Pred::AllSatisfy("list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": []})));
    // Non-array → false.
    assert!(!Pred::AllSatisfy("list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": 42})));
    // Unknown predicate name → fail-closed (false).
    assert!(!Pred::AllSatisfy("list".to_string(), "UnknownPred".to_string())
        .eval(&json!({"list": [1, 2, 3]})));
}

#[test]
fn pred_eval_forall() {
    // Forall behaves like AllSatisfy.
    assert!(Pred::Forall("x".to_string(), "list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": [[1, 2], [3, 4]]})));
    assert!(!Pred::Forall("x".to_string(), "list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": [[1, 2], [4, 3]]})));
    // Non-array → false.
    assert!(!Pred::Forall("x".to_string(), "list".to_string(), "Sorted".to_string())
        .eval(&json!({"list": 42})));
}

#[test]
fn pred_eval_single_var_fallback_to_whole_value() {
    // When the variable name is not a key in an object, fall back to the
    // whole value. So Sorted("output") against a bare array checks the array.
    assert!(Pred::Sorted("output".to_string()).eval(&json!([1, 2, 3])));
    assert!(!Pred::Sorted("output".to_string()).eval(&json!([3, 1, 2])));
    // Le("output", "1") against a bare number: "output" falls back to the
    // number, "1" also falls back to the number, so x <= x → true.
    assert!(Pred::Le("output".to_string(), "1".to_string()).eval(&json!(5)));
}

// ============================================================================
// ContractChecker tests
// ============================================================================

#[test]
fn contract_checker_requires_rejects_violating_input() {
    // precondition: Le("a", "b") — a <= b.
    let contract = Contract::conditional(
        RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Int)),
            Box::new(RefTy::Plain(BaseTy::Int)),
        ),
        RefTy::Plain(BaseTy::Int),
        Pred::Le("a".to_string(), "b".to_string()),
        Pred::True,
        None,
    );
    let checker = ContractChecker::new("test.op", contract);

    // Single-arg: the arg is an object {a, b}. a=1 <= b=2 → ok.
    checker.check_requires(&[json!({"a": 1, "b": 2})]).unwrap();
    // a=3 > b=2 → violation.
    let err = checker.check_requires(&[json!({"a": 3, "b": 2})]).unwrap_err();
    assert!(err.contains("requires predicate"), "got: {err}");
    assert!(err.contains("test.op"));
}

#[test]
fn contract_checker_requires_true_always_passes() {
    let contract = Contract::new(
        RefTy::Plain(BaseTy::Int),
        RefTy::Plain(BaseTy::Int),
        Pred::True,
        None,
    );
    let checker = ContractChecker::new("test.op", contract);
    // Any input passes when precondition is True.
    checker.check_requires(&[json!(42)]).unwrap();
    checker.check_requires(&[json!("hello")]).unwrap();
    checker.check_requires(&[]).unwrap();
}

#[test]
fn contract_checker_guarantee_rejects_violating_output() {
    // guarantee: Sorted("output") — output is a sorted array.
    let contract = Contract::new(
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
        Pred::Sorted("output".to_string()),
        None,
    );
    let checker = ContractChecker::new("test.sort", contract);

    // Sorted output → ok.
    checker.check_guarantee(&json!([1, 2, 3])).unwrap();
    // Unsorted output → violation.
    let err = checker.check_guarantee(&json!([3, 1, 2])).unwrap_err();
    assert!(err.contains("guarantee predicate"), "got: {err}");
    assert!(err.contains("test.sort"));
}

#[test]
fn contract_checker_guarantee_true_always_passes() {
    let contract = Contract::new(
        RefTy::Plain(BaseTy::Int),
        RefTy::Plain(BaseTy::Int),
        Pred::True,
        None,
    );
    let checker = ContractChecker::new("test.op", contract);
    checker.check_guarantee(&json!(42)).unwrap();
    checker.check_guarantee(&json!("anything")).unwrap();
}

#[test]
fn contract_checker_multi_arg_requires() {
    // precondition: Le("0", "1") — first arg <= second arg (array index
    // lookup on the multi-arg encoded array).
    let contract = Contract::conditional(
        RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Int)),
            Box::new(RefTy::Plain(BaseTy::Int)),
        ),
        RefTy::Plain(BaseTy::Int),
        Pred::Le("0".to_string(), "1".to_string()),
        Pred::True,
        None,
    );
    let checker = ContractChecker::new("test.add", contract);

    // Two args: 1 <= 2 → ok.
    checker.check_requires(&[json!(1), json!(2)]).unwrap();
    // Two args: 5 > 2 → violation.
    let err = checker.check_requires(&[json!(5), json!(2)]).unwrap_err();
    assert!(err.contains("requires predicate"), "got: {err}");
}

// ============================================================================
// register_contract_checkers tests
// ============================================================================

/// A mock `Function` that sorts a JSON array of numbers.
struct SortFn;
impl Function for SortFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let arr = args[0]
            .as_array()
            .ok_or_else(|| EvalError::TypeMismatch("expected array".to_string()))?;
        let mut nums: Vec<f64> = arr
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect();
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Ok(json!(nums))
    }
}

/// A mock `Function` that returns an unsorted array (to trigger guarantee
/// violations).
struct UnsortedFn;
impl Function for UnsortedFn {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Ok(json!([3, 1, 2]))
    }
}

#[test]
fn register_contract_checkers_wires_checkers() {
    // Build a contract registry with a "test.sort" op that guarantees
    // Sorted(output).
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.sort",
        ArrSource::Native,
        Contract::new(
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
            Pred::Sorted("output".to_string()),
            None,
        ),
    );

    // Build an op registry with the function registered.
    let mut registry = OpRegistry::new();
    registry.register("test.sort", Arc::new(SortFn));

    // Wire the contract checkers.
    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 1, "one checker should be attached");
    assert!(registry.get_checker("test.sort").is_some());

    // Verify the checker fires via OpFunction::call.
    let shared = Arc::new(registry);
    let op_func = OpFunction::new("test.sort", Arc::clone(&shared));
    let mut ctx = EvalContext::new();

    // Sorted input → sorted output → guarantee passes.
    let result = op_func.call(&[json!([3, 1, 2])], &mut ctx).unwrap();
    assert_eq!(result, json!([1.0, 2.0, 3.0]));
}

#[test]
fn register_contract_checkers_guarantee_violation_rejected() {
    // Build a contract registry with a "test.bad_sort" op that guarantees
    // Sorted(output), but the function returns an unsorted array.
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.bad_sort",
        ArrSource::Native,
        Contract::new(
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Int))),
            Pred::Sorted("output".to_string()),
            None,
        ),
    );

    let mut registry = OpRegistry::new();
    registry.register("test.bad_sort", Arc::new(UnsortedFn));

    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 1);

    let shared = Arc::new(registry);
    let op_func = OpFunction::new("test.bad_sort", Arc::clone(&shared));
    let mut ctx = EvalContext::new();

    // The function returns [3, 1, 2] which is unsorted → guarantee violated.
    let err = op_func.call(&[json!([1, 2, 3])], &mut ctx).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("guarantee predicate violated"),
        "expected guarantee violation, got: {msg}"
    );
}

#[test]
fn register_contract_checkers_skips_unregistered_ops() {
    // Contract registry has an op, but no function is registered in the
    // OpRegistry → the checker is not attached (count == 0).
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.unregistered",
        ArrSource::Native,
        Contract::new(
            RefTy::Plain(BaseTy::Int),
            RefTy::Plain(BaseTy::Int),
            Pred::True,
            None,
        ),
    );

    let mut registry = OpRegistry::new();
    // No function registered for "test.unregistered".
    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 0, "no checker should be attached without a function");
    assert!(registry.get_checker("test.unregistered").is_none());
}

#[test]
fn register_contract_checkers_skips_judgment_source() {
    // Judgment source contracts are skipped (typecheck-time only).
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.judgment",
        ArrSource::Judgment,
        Contract::new(
            RefTy::Plain(BaseTy::Int),
            RefTy::Plain(BaseTy::Int),
            Pred::True,
            None,
        ),
    );

    let mut registry = OpRegistry::new();
    registry.register("test.judgment", Arc::new(SortFn));

    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 0, "Judgment source contracts should be skipped");
    assert!(registry.get_checker("test.judgment").is_none());
}

#[test]
fn register_contract_checkers_requires_enforced_via_op_call() {
    // precondition: Le("a", "b") — enforced via OpFunction::call.
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.le_op",
        ArrSource::Native,
        Contract::conditional(
            RefTy::Prod(
                Box::new(RefTy::Plain(BaseTy::Int)),
                Box::new(RefTy::Plain(BaseTy::Int)),
            ),
            RefTy::Plain(BaseTy::Int),
            Pred::Le("a".to_string(), "b".to_string()),
            Pred::True,
            None,
        ),
    );

    // A function that takes a single object arg {a, b} and returns a + b.
    struct AddFn;
    impl Function for AddFn {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let obj = &args[0];
            let a = obj.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = obj.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!(a + b))
        }
    }

    let mut registry = OpRegistry::new();
    registry.register("test.le_op", Arc::new(AddFn));

    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 1);

    let shared = Arc::new(registry);
    let op_func = OpFunction::new("test.le_op", Arc::clone(&shared));
    let mut ctx = EvalContext::new();

    // a=1 <= b=2 → requires passes.
    let result = op_func.call(&[json!({"a": 1, "b": 2})], &mut ctx).unwrap();
    assert_eq!(result, json!(3.0));

    // a=5 > b=2 → requires violated.
    let err = op_func.call(&[json!({"a": 5, "b": 2})], &mut ctx).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("requires predicate violated"),
        "expected requires violation, got: {msg}"
    );
}

#[test]
fn register_contract_checkers_multiple_ops() {
    // Register multiple contracts and verify all get checkers.
    let mut contracts = ContractRegistry::new();
    contracts.register(
        "test.op_a",
        ArrSource::Native,
        Contract::new(
            RefTy::Plain(BaseTy::Int),
            RefTy::Plain(BaseTy::Int),
            Pred::True,
            None,
        ),
    );
    contracts.register(
        "test.op_b",
        ArrSource::Native,
        Contract::new(
            RefTy::Plain(BaseTy::Int),
            RefTy::Plain(BaseTy::Int),
            Pred::True,
            None,
        ),
    );

    let mut registry = OpRegistry::new();
    registry.register("test.op_a", Arc::new(SortFn));
    registry.register("test.op_b", Arc::new(SortFn));

    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 2);
    assert!(registry.get_checker("test.op_a").is_some());
    assert!(registry.get_checker("test.op_b").is_some());
}

#[test]
fn register_contract_checkers_empty_registry() {
    let contracts = ContractRegistry::new();
    let mut registry = OpRegistry::new();
    let count = register_contract_checkers(&mut registry, &contracts);
    assert_eq!(count, 0);
}
