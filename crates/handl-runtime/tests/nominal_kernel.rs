//! Nominal identity VM primitives (ADR-0106 §2, Phase 2 kernel).
//!
//! Covers the `sym()` / `sym(parent)` and `instance_of(value, nominal)`
//! host functions of [`VmPrimitivesModule`]: freshness and identity
//! equality, the `sym(A)` error contract when `A` is not nominal,
//! instanceof (direct / transitive / negative / non-nominal value), and
//! serialization round-trips of nominal values through the
//! `{"__sym__": ...}` JSON encoding.

use handl_platform::{EvalContext, Function, Module};
use handl_runtime::vm_primitives::VmPrimitivesModule;
use handl_value::{TaggedValue, eq};
use serde_json::{Value, json};

/// Call a VM primitive by name, panicking on error (for success-path tests).
fn call(name: &str, args: &[Value]) -> Value {
    let module = VmPrimitivesModule::new();
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("vm_primitives has no '{}' function", name));
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
        .unwrap_or_else(|e| panic!("vm primitive '{}' error: {:?}", name, e))
}

/// Call a VM primitive by name, returning the error (for error-path tests).
fn call_err(name: &str, args: &[Value]) -> handl_platform::EvalError {
    let module = VmPrimitivesModule::new();
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("vm_primitives has no '{}' function", name));
    let mut ctx = EvalContext::new();
    match func.call(args, &mut ctx) {
        Ok(v) => panic!("vm primitive '{}' unexpectedly succeeded: {:?}", name, v),
        Err(e) => e,
    }
}

/// Convert a VM result into a `TaggedValue` for equality checks.
fn as_tagged(v: Value) -> TaggedValue {
    TaggedValue::from(v)
}

/// True iff the two JSON values decode to equal (identity-equal) nominals.
fn nominal_eq_json(a: &Value, b: &Value) -> bool {
    eq(&as_tagged(a.clone()), &as_tagged(b.clone()))
}

// ---------------------------------------------------------------------------
// sym() freshness: two syms unequal; equal to themselves; hash/eq consistency
// ---------------------------------------------------------------------------

#[test]
fn vm_sym_fresh_two_calls_unequal_same_name() {
    let s1 = call("sym", &[]);
    let s2 = call("sym", &[]);
    // Both display the same presentation name ...
    assert_eq!(
        s1.get("__sym__").unwrap().get("name").unwrap(),
        s2.get("__sym__").unwrap().get("name").unwrap(),
        "names are presentation only"
    );
    // ... but identity is the handle, so they are distinct syms.
    assert!(!nominal_eq_json(&s1, &s2));
    assert_ne!(
        s1.get("__sym__").unwrap().get("handle").unwrap(),
        s2.get("__sym__").unwrap().get("handle").unwrap()
    );
}

#[test]
fn vm_sym_equal_to_itself_and_hash_consistent() {
    let s = call("sym", &[]);
    let s_copy = s.clone();
    assert!(nominal_eq_json(&s, &s_copy));
    // hash/eq consistency via TaggedValue's Hash.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let t1 = as_tagged(s.clone());
    let t2 = as_tagged(s_copy);
    let mut h1 = DefaultHasher::new();
    t1.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    t2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn vm_sym_root_has_null_parent() {
    let s = call("sym", &[]);
    let inner = s.get("__sym__").unwrap();
    assert!(inner.get("parent").unwrap().is_null());
    assert!(inner.get("handle").unwrap().as_u64().unwrap() > 0);
}

// ---------------------------------------------------------------------------
// sym(parent): parent chains, name collisions, errors
// ---------------------------------------------------------------------------

/// Build a VM-level 3-chain: Obj <- Number <- Float. Returns (root, number,
/// float) as JSON values.
fn vm_chain() -> (Value, Value, Value) {
    let root = call("sym", &[]);
    let number = call("sym", &[root.clone()]);
    let float = call("sym", &[number.clone()]);
    (root, number, float)
}

#[test]
fn vm_sym_parent_chain_is_explicit_in_json() {
    let (root, number, float) = vm_chain();
    // sym(A) nests the parent reference explicitly.
    let inner = number.get("__sym__").unwrap();
    let parent = inner.get("parent").unwrap();
    assert_eq!(
        parent.get("__sym__").unwrap().get("handle").unwrap(),
        root.get("__sym__").unwrap().get("handle").unwrap()
    );
    // The float's parent is the number (deep chain).
    let fparent = float.get("__sym__").unwrap().get("parent").unwrap();
    assert_eq!(
        fparent.get("__sym__").unwrap().get("handle").unwrap(),
        inner.get("handle").unwrap()
    );
    // Parent appears exactly once (single parent, diamond-free).
    let fparent_str = fparent.to_string();
    let number_str = number.to_string();
    let number_handle = inner.get("handle").unwrap().to_string();
    assert_eq!(
        fparent_str
            .matches(&format!("\"handle\":{}", number_handle))
            .count(),
        1,
        "parent chain is linear — the parent handle appears exactly once"
    );
    // (sanity: the number's own JSON also encodes its parent exactly once)
    assert!(number_str.contains("\"parent\""));
}

#[test]
fn vm_sym_same_name_distinct_syms_unequal() {
    // Two syms with identical display names remain unequal: identity is
    // handle-based, names are presentation only.
    let a = call("sym", &[]);
    let b = call("sym", &[]);
    assert_eq!(
        a.get("__sym__").unwrap().get("name").unwrap(),
        b.get("__sym__").unwrap().get("name").unwrap()
    );
    assert!(!nominal_eq_json(&a, &b));
}

#[test]
fn vm_sym_non_nominal_parent_errors() {
    // sym(A) where A is not a nominal -> clean error.
    for bad in [
        json!(42),
        json!("str"),
        json!(null),
        json!([1, 2]),
        json!({"a": 1}),
        json!({"__tag__": "int", "__registry__": "type", "kind": "primitive"}),
    ] {
        let err = call_err("sym", &[bad]);
        let msg = format!("{:?}", err);
        assert!(msg.contains("parent is not a nominal object"), "{}", msg);
    }
}

#[test]
fn vm_sym_too_many_args_errors() {
    let (root, _number, _float) = vm_chain();
    let err = call_err("sym", &[root, json!(1)]);
    let msg = format!("{:?}", err);
    assert!(msg.contains("expected 0 or 1 argument"), "{}", msg);
}

// ---------------------------------------------------------------------------
// instance_of: direct, transitive, negative, non-nominal value
// ---------------------------------------------------------------------------

#[test]
fn vm_instance_of_direct_and_self() {
    let (_root, _number, _float) = vm_chain();
    // Direct: Float instanceof Number is built below; here Number instanceof
    // Number (self) and Number instanceof root (direct parent).
    let (_r, n, _f) = vm_chain();
    let root = n.get("__sym__").unwrap().get("parent").unwrap().clone();
    assert_eq!(call("instance_of", &[n.clone(), n.clone()]), json!(true));
    assert_eq!(call("instance_of", &[n.clone(), root.clone()]), json!(true));
}

#[test]
fn vm_instance_of_transitive() {
    let (_root, _number, float) = vm_chain();
    let root = float
        .get("__sym__")
        .unwrap()
        .get("parent")
        .unwrap()
        .get("__sym__")
        .unwrap()
        .get("parent")
        .unwrap()
        .clone();
    // Transitive: Float instanceof root (grandparent).
    assert_eq!(
        call("instance_of", &[float.clone(), root.clone()]),
        json!(true)
    );
}

#[test]
fn vm_instance_of_negative_parent_to_child_and_siblings() {
    let (root, number, float) = vm_chain();
    // Parent -> child is not an instance.
    assert_eq!(
        call("instance_of", &[number.clone(), float.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[root.clone(), float.clone()]),
        json!(false)
    );
    // Sibling syms (same parent) are unrelated.
    let sibling = call("sym", &[root.clone()]);
    assert_eq!(
        call("instance_of", &[sibling.clone(), number.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[number.clone(), sibling.clone()]),
        json!(false)
    );
}

#[test]
fn vm_instance_of_non_nominal_value_is_false() {
    let (_root, number, _float) = vm_chain();
    // A non-nominal value is simply not an instance of a nominal: false,
    // no error.
    for v in [
        json!(42),
        json!("s"),
        json!(null),
        json!([1]),
        json!({"a": 1}),
    ] {
        assert_eq!(
            call("instance_of", &[v.clone(), number.clone()]),
            json!(false)
        );
    }
}

#[test]
fn vm_instance_of_non_nominal_nominal_arg_errors() {
    let (root, _number, _float) = vm_chain();
    // instance_of(value, <not a nominal>) -> clean error.
    for bad in [json!(42), json!(null), json!({"a": 1})] {
        let err = call_err("instance_of", &[root.clone(), bad]);
        let msg = format!("{:?}", err);
        assert!(msg.contains("must be a nominal object"), "{}", msg);
    }
}

#[test]
fn vm_instance_of_wrong_arity_errors() {
    let err = call_err("instance_of", &[json!(1)]);
    let msg = format!("{:?}", err);
    assert!(msg.contains("expected two arguments"), "{}", msg);
}

// ---------------------------------------------------------------------------
// Phase 4 legacy-tag bridge (ADR-0106 migration note): tag→nominal mapping.
// Datum `__tag__` ⇒ nominal role: int⇒Int, float⇒Float, nat⇒Nat (via Int),
// number⇒Number. `sym()` mints anonymous ("sym") parents here, so the
// bridge resolves by depth — hand-built chains below use named syms.
// ---------------------------------------------------------------------------

/// Build a named standard chain: Obj <- Number <- {Int <- Nat, Float}.
/// Names matter: the bridge separates Int-branch from Float-branch via
/// presentation names (`chain_node_branch`).
fn named_chain() -> (Value, Value, Value, Value) {
    let obj = json!({"__sym__": {"handle": 101, "name": "Obj", "parent": null}});
    let number = json!({"__sym__": {"handle": 102, "name": "Number", "parent": obj}});
    let int = json!({"__sym__": {"handle": 103, "name": "Int", "parent": number.clone()}});
    let float = json!({"__sym__": {"handle": 104, "name": "Float", "parent": number.clone()}});
    let nat = json!({"__sym__": {"handle": 105, "name": "Nat", "parent": int.clone()}});
    (number, int, float, nat)
}

/// A legacy `type.hnl`-style datum: `{"__tag__": tag, ...}`.
fn tagged_datum(tag: &str) -> Value {
    json!({"__tag__": tag, "__registry__": "type", "kind": "primitive"})
}

#[test]
fn vm_instance_of_legacy_float_datum_vs_number() {
    let (number, _, _, _) = named_chain();
    assert_eq!(
        call("instance_of", &[tagged_datum("float"), number.clone()]),
        json!(true)
    );
}

#[test]
fn vm_instance_of_legacy_int_datum_vs_number() {
    let (number, _, _, _) = named_chain();
    assert_eq!(
        call("instance_of", &[tagged_datum("int"), number.clone()]),
        json!(true)
    );
}

#[test]
fn vm_instance_of_legacy_nat_datum_vs_int_and_number() {
    let (number, int, _, _) = named_chain();
    assert_eq!(
        call("instance_of", &[tagged_datum("nat"), int.clone()]),
        json!(true)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("nat"), number.clone()]),
        json!(true)
    );
}

#[test]
fn vm_instance_of_legacy_negative_sibling_and_reverse() {
    let (number, int, float, _) = named_chain();
    assert_eq!(
        call("instance_of", &[tagged_datum("int"), float.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("float"), int.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("number"), int.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("number"), float.clone()]),
        json!(false)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("number"), number.clone()]),
        json!(true)
    );
}

#[test]
fn vm_instance_of_legacy_unknown_tags_and_plain_values_untouched() {
    let (number, _, _, _) = named_chain();
    for v in [
        tagged_datum("str"),
        tagged_datum("bool"),
        tagged_datum("nope"),
        json!(42),
        json!("s"),
        json!(null),
    ] {
        assert_eq!(
            call("instance_of", &[v.clone(), number.clone()]),
            json!(false)
        );
    }
}

#[test]
fn vm_instance_of_legacy_bridges_by_name_not_subtype_or_anonymous() {
    let (number, int, float, _) = named_chain();
    // Case-insensitive presentation-name match.
    let lower = json!({"__sym__": {"handle": 201, "name": "number", "parent": null}});
    assert_eq!(
        call("instance_of", &[tagged_datum("int"), lower]),
        json!(true)
    );
    // `instanceof` is "is-a", not "has-subtype": an `Int` datum is not an
    // instance of a nominal *derived from* Int (MyInt := sym(Int)).
    let my_int = json!({"__sym__": {"handle": 202, "name": "MyInt", "parent": int.clone()}});
    assert_eq!(
        call("instance_of", &[tagged_datum("int"), my_int]),
        json!(false)
    );
    // A named-less nominal (`sym()` anonymous) never bridges a legacy datum.
    let anon = json!({"__sym__": {"handle": 203, "name": "sym", "parent": number}});
    assert_eq!(
        call("instance_of", &[tagged_datum("float"), anon]),
        json!(false)
    );
    // Sanity: the genuine nominals still answer via their own names.
    assert_eq!(
        call("instance_of", &[tagged_datum("nat"), int]),
        json!(true)
    );
    assert_eq!(
        call("instance_of", &[tagged_datum("nat"), float]),
        json!(false)
    );
}
