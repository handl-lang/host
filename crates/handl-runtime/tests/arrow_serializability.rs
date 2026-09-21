//! ADR-0106 §6/§8 — arrow serializability conformance tests (Phase 5).
//!
//! Arrows are **objects**: serializable, inspectable, self-contained data
//! with no closures, no lexical environments, and no hidden captured state.
//! `TaggedValue::Arrow { params, rest, array_pattern, body_def, binds,
//! handlers }` (`crates/handl-value/src/lib.rs`) carries:
//!
//! - `body_def` — the body as explicit serializable structure
//!   (`{"__ast__": <Ast>}`), with free variables already substituted as
//!   literal nodes (`handl_language::instantiate`). The arrow's *meaning*
//!   travels with the value; no process-local frame is consulted.
//! - `binds` — the residual explicit binding map: names that cannot be
//!   substituted (recursive-region binders, whose fixpoint pass must see
//!   them free, and non-serializable values like `Resource`).
//! - `handlers` — carried handlers attached by `handle <arrow> with <h>`,
//!   explicit data instead of the old `env["__handlers__"]` channel.
//!
//! The pre-Phase-5 representation (`body` arena index + `env` capture map +
//! `__home__` frame id) is gone: there is no `HOME_FRAMES` registry, no
//! `__envref__`, and nothing process-local on the arrow.
//!
//! These tests pin down:
//!
//! 1. the JSON encoding of an arrow uses exactly the documented key
//!    universe (`__arrow__`, `params`, `body_def`, `rest`,
//!    `array_pattern`, `binds`, `handlers`) — silent additions fail loudly;
//! 2. a bare arrow round-trips JSON -> value -> JSON unchanged;
//! 3. an arrow whose `binds` holds a nested arrow round-trips **exactly**
//!    at the value level;
//! 4. an arrow whose `binds` holds a `Resource` round-trips **lossily**
//!    (the finalizer is dropped — see the test's comment);
//! 5. a composed arrow (`a >>> b`, represented as the `Seq` term) has no
//!    hidden state beyond its two component terms;
//! 6. the arrow the engine actually mints carries `body_def` and no
//!    legacy `body`/`env`/`__home__` keys;
//! 7. `body_def`/`binds`/`handlers` are the arrow's *identity* — equality
//!    and hashing include them;
//! 8. a JSON-round-tripped arrow applies correctly in a runtime that never
//!    saw the minting one;
//! 9. referenced bindings are substituted into `body_def`, not captured.

use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_value::{TaggedValue, resource, tagged_to_value};
use plugin_engine::{Term, TermData};
use rustc_hash::FxHashMap;
use serde_json::{Value, json};

/// The documented JSON key universe for an arrow value (ADR-0106 §6; the
/// `TaggedValue::Arrow` variant fields of `handl-value/src/lib.rs`).
const DOCUMENTED_ARROW_KEYS: &[&str] = &[
    "__arrow__",
    "params",
    "body_def",
    "rest",
    "array_pattern",
    "binds",
    "handlers",
];

/// The tagged `{"__ast__": <Ast>}` shape `body_def` carries.
fn body_def_json() -> Value {
    json!({
        "__ast__": {
            "nodes": [{ "kind": "var", "span": null, "payload": { "name": "x" } }]
        }
    })
}

/// Round-trip a JSON value through the tagged-value representation.
fn round_trip(v: Value) -> Value {
    let t: TaggedValue = v.into();
    tagged_to_value(&t)
}

/// A bare arrow (`x ~> x`) — the shape the engines mint.
fn bare_arrow_json() -> Value {
    json!({ "__arrow__": true, "params": ["x"], "body_def": body_def_json() })
}

// ---------------------------------------------------------------------------
// 1. The JSON shape is pinned to the documented key universe.
// ---------------------------------------------------------------------------

/// A bare arrow emits **exactly** the three mandatory keys — no more. New
/// keys added to the encoder fail this test loudly (pin the shape).
#[test]
fn bare_arrow_json_has_exactly_the_mandatory_keys() {
    let out = round_trip(bare_arrow_json());
    let obj = out.as_object().expect("arrow encodes as a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["__arrow__", "body_def", "params"],
        "bare arrow key set changed; if a new field is intentional, extend \
         DOCUMENTED_ARROW_KEYS and the ADR-0106 Phase 5 conformance note",
    );
    // The pre-Phase-5 fields are gone entirely.
    for legacy in ["body", "env", "__home__", "home"] {
        assert!(
            !obj.contains_key(legacy),
            "legacy arrow key `{legacy}` must not be emitted",
        );
    }
}

/// The optional keys (`rest`, `array_pattern`, `binds`, `handlers`) appear
/// only when set, and every emitted key is inside the documented universe.
#[test]
fn arrow_json_keys_are_a_subset_of_the_documented_universe() {
    // Full arrow: every optional field present.
    let full = json!({
        "__arrow__": true,
        "params": ["a"],
        "rest": "more",
        "array_pattern": true,
        "body_def": body_def_json(),
        "binds": { "self_ref": 1 },
        "handlers": [ { "__tag__": "Some", "value": 0 } ],
    });
    let out = round_trip(full.clone());
    assert_eq!(out, full, "full arrow round-trips unchanged");

    let obj = out.as_object().expect("arrow encodes as a JSON object");
    for key in obj.keys() {
        assert!(
            DOCUMENTED_ARROW_KEYS.contains(&key.as_str()),
            "undocumented arrow JSON key `{key}` (documented: {DOCUMENTED_ARROW_KEYS:?})",
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Bare arrow round-trips JSON -> value -> JSON unchanged.
// ---------------------------------------------------------------------------

#[test]
fn bare_arrow_round_trips_unchanged() {
    for arrow in [
        bare_arrow_json(),
        json!({ "__arrow__": true, "params": ["x", "y"], "body_def": body_def_json() }),
    ] {
        assert_eq!(round_trip(arrow.clone()), arrow);
    }
}

/// Round-tripping is stable across two passes (idempotent) for a bare arrow.
#[test]
fn bare_arrow_round_trip_is_idempotent() {
    let once = round_trip(bare_arrow_json());
    let twice = round_trip(once.clone());
    assert_eq!(once, twice);
}

// ---------------------------------------------------------------------------
// 3. Nested arrow in `binds` — round-trips exactly.
// ---------------------------------------------------------------------------

/// An arrow whose `binds` holds a *nested arrow* round-trips **exactly**:
/// both arrows are self-contained data (`body_def` + `binds`), so nothing
/// process-local is referenced on either side of the boundary.
#[test]
fn binds_nested_arrow_round_trips_exactly() {
    let outer = json!({
        "__arrow__": true,
        "params": ["x"],
        "body_def": body_def_json(),
        "binds": {
            "inner": {
                "__arrow__": true,
                "params": ["y"],
                "body_def": body_def_json(),
            }
        },
    });
    let out = round_trip(outer.clone());
    assert_eq!(
        out, outer,
        "nested bound arrow must round-trip byte-for-byte"
    );
    assert_eq!(
        out["binds"]["inner"]["params"],
        json!(["y"]),
        "the nested arrow's data is preserved on the JSON side",
    );
}

// ---------------------------------------------------------------------------
// 4. Resource in `binds` — round-trips *lossily*.
// ---------------------------------------------------------------------------

/// Set true by the finalizer below.
static FINALIZED: AtomicBool = AtomicBool::new(false);

unsafe fn mark_finalized(_ptr: *mut c_void) {
    FINALIZED.store(true, Ordering::SeqCst);
}

/// An arrow whose `binds` holds a `Resource` (a foreign RAII handle)
/// round-trips **lossily**.
///
/// Precise result recorded by this test:
/// - the JSON encoding is the debug-only `{"__resource__": {"ptr": N}}` —
///   it carries the pointer but **drops the finalizer** (a function
///   pointer, not serializable);
/// - the decoded `Resource` has `finalizer = None`, so dropping the
///   round-tripped arrow does **not** run the original finalizer — the
///   resource leaks. `binds` exists precisely so non-substitutable values
///   like this are *visible* in the arrow's data rather than hidden in a
///   captured environment.
#[test]
fn binds_resource_round_trips_lossily() {
    // Build a *live* resource carrying a finalizer, then put it in an
    // arrow's binds. `ptr = 1` is never dereferenced (opaque handle).
    let live = unsafe { resource(1 as *mut c_void, Some(mark_finalized)) };
    let mut binds: FxHashMap<Rc<str>, TaggedValue> = FxHashMap::default();
    binds.insert(Rc::from("handle"), live);
    let arrow = TaggedValue::Arrow {
        params: Rc::from(vec!["x".to_string()]),
        rest: None,
        array_pattern: false,
        body_def: Rc::new(bare_arrow_json()["body_def"].clone().into()),
        binds: Rc::new(binds),
        handlers: Rc::from(Vec::<TaggedValue>::new()),
    };

    // Serialize: the resource becomes a debug-only pointer record, no finalizer.
    let encoded = tagged_to_value(&arrow);
    assert_eq!(
        encoded["binds"]["handle"],
        json!({ "__resource__": { "ptr": 1 } }),
        "resource encodes as a debug-only pointer record",
    );

    // Round-trip the *encoded* form: the decoded resource has no finalizer.
    FINALIZED.store(false, Ordering::SeqCst);
    let decoded: TaggedValue = encoded.clone().into();
    match &decoded {
        TaggedValue::Arrow { binds, .. } => {
            let handle = binds.get("handle").expect("decoded binds keeps `handle`");
            assert!(
                matches!(handle, TaggedValue::Resource(_)),
                "decoded binds value is a Resource",
            );
        }
        other => panic!("decoded arrow is not an Arrow value: {other:?}"),
    }
    drop(decoded);
    assert!(
        !FINALIZED.load(Ordering::SeqCst),
        "the round-tripped (finalizer-less) resource must NOT finalize — \
         the finalizer was dropped by serialization",
    );

    // Dropping the *original* arrow does run the finalizer, which confirms the
    // finalizer was live before serialization.
    drop(arrow);
    assert!(
        FINALIZED.load(Ordering::SeqCst),
        "the original resource finalizes when its arrow drops",
    );
}

// ---------------------------------------------------------------------------
// 5. Composed arrow (`a >>> b`) has no hidden state beyond its components.
// ---------------------------------------------------------------------------

/// Composition is represented as the `Seq` **term** — `{"__tag__": "Seq",
/// "axis": 0, "fst": <a>, "snd": <b>}` — a plain serializable tagged record
/// with only its axis slot and two components; the encoder adds nothing
/// else.
///
/// The `axis` field is the composition axis (`>>>[k]`, ADR-0120 Phase 2);
/// `>>>` is `>>>[0]`. Serialized `Seq` records written before Phase 2 (no
/// `axis` key) still decode with the default axis 0 — see
/// `legacy_seq_record_without_axis_decodes` below.
#[test]
fn composed_seq_term_has_only_component_fields() {
    let seq = TermData::seq(
        TermData::arr(bare_arrow_json()),
        TermData::arr(json!({"__arrow__": true, "params": ["y"], "body_def": body_def_json()})),
    );
    let obj = seq
        .0
        .as_object()
        .expect("Seq term encodes as a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["__tag__", "axis", "fst", "snd"],
        "the Seq composition record must carry only its axis and two components",
    );
    // `>>>` is `>>>[0]`: the default axis encodes as the `Const 0` term.
    assert_eq!(
        obj["axis"],
        json!({"__tag__": "Const", "value": 0}),
        "default Seq axis is the Const 0 term",
    );
    // The components are themselves the (serializable) arrow records, each
    // wrapped as the `value` of an `Arr` term.
    assert_eq!(obj["fst"]["value"]["__arrow__"], json!(true));
    assert_eq!(obj["snd"]["value"]["__arrow__"], json!(true));
    // And the whole record round-trips unchanged.
    assert_eq!(round_trip(seq.0.clone()), seq.0);
}

/// A `Seq` record serialized before ADR-0120 Phase 2 — `{__tag__, fst,
/// snd}` with no `axis` — decodes with the default composition axis 0.
#[test]
fn legacy_seq_record_without_axis_decodes() {
    let legacy = json!({
        "__tag__": "Seq",
        "fst": {"__tag__": "Const", "value": 1},
        "snd": {"__tag__": "Const", "value": 2}
    });
    let term = TermData(legacy).to_term().expect("legacy Seq decodes");
    match term {
        Term::Seq { axis, fst, snd } => {
            assert!(axis.is_default_axis(), "missing axis defaults to 0");
            assert!(matches!(*fst, Term::Const { .. }));
            assert!(matches!(*snd, Term::Const { .. }));
        }
        other => panic!("expected Seq, got {other:?}"),
    }
}

/// `seq_axis` writes the axis slot; a `Seq` carrying axis `k` round-trips
/// with the axis intact (ADR-0120 Phase 2).
#[test]
fn seq_axis_record_round_trips_with_axis() {
    let seq = TermData::seq_axis(
        TermData::const_value(2),
        TermData::arr(bare_arrow_json()),
        TermData::arr(json!({"__arrow__": true, "params": ["y"], "body_def": body_def_json()})),
    );
    let term = seq.to_term().expect("seq_axis decodes");
    match term {
        Term::Seq { axis, .. } => {
            assert_eq!(
                axis.nat_literal(),
                Some(2),
                "axis 2 survives the round trip"
            );
        }
        other => panic!("expected Seq, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 6. The arrow the engine actually mints has the documented shape.
// ---------------------------------------------------------------------------

/// The tree-walk engine, asked to evaluate `x ~> x`, returns an arrow whose
/// JSON keys are all inside the documented universe, whose `params` is
/// `["x"]`, and whose `body_def` is the serialized standalone AST arena —
/// no `body` index, no `env`, no `__home__`.
#[test]
fn engine_arrow_value_uses_documented_shape() {
    let rt = canonical_runtime();
    let empty: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let arrow = eval_in(&rt, "x ~> x", &empty);

    assert_eq!(arrow["__arrow__"], json!(true), "result is an arrow value");
    assert_eq!(arrow["params"], json!(["x"]));

    // ADR-0106 Phase 5: the body is explicit serializable structure.
    let body_def = arrow
        .get("body_def")
        .expect("a minted arrow carries `body_def`");
    assert!(
        body_def.get("__ast__").is_some(),
        "`body_def` is the language-level AST value `{{\"__ast__\": <Ast>}}`: {body_def}",
    );

    // The pre-Phase-5 fields are gone.
    for legacy in ["body", "env", "__home__", "home"] {
        assert!(
            arrow.get(legacy).is_none(),
            "engine produced legacy arrow key `{legacy}`",
        );
    }

    let obj = arrow.as_object().expect("arrow encodes as a JSON object");
    for key in obj.keys() {
        assert!(
            DOCUMENTED_ARROW_KEYS.contains(&key.as_str()),
            "engine produced an undocumented arrow key `{key}`",
        );
    }

    // The engine-produced arrow round-trips through the value model unchanged.
    assert_eq!(round_trip(arrow.clone()), arrow);
}

// ---------------------------------------------------------------------------
// 7. `body_def`/`binds`/`handlers` are the arrow's identity.
// ---------------------------------------------------------------------------

/// Unlike the old derived `body_def`, the standalone definition *is* the
/// arrow: equality and hashing include `body_def`, `binds`, and `handlers`,
/// so two arrows are equal iff their serializable data is equal.
#[test]
fn arrow_identity_is_its_serializable_data() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(v: &TaggedValue) -> u64 {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    let a: TaggedValue = bare_arrow_json().into();
    let a2: TaggedValue = bare_arrow_json().into();
    assert_eq!(a, a2, "identical arrow data is equal");
    assert_eq!(hash_of(&a), hash_of(&a2), "Hash agrees with Eq");

    // Different body data → different arrow.
    let other_body: TaggedValue = json!({
        "__arrow__": true,
        "params": ["x"],
        "body_def": { "__ast__": { "nodes": [] } }
    })
    .into();
    assert_ne!(
        a, other_body,
        "arrows with different `body_def` are different values",
    );

    // A bound name is part of the identity.
    let with_binds: TaggedValue = json!({
        "__arrow__": true,
        "params": ["x"],
        "body_def": body_def_json(),
        "binds": { "self_ref": 1 }
    })
    .into();
    assert_ne!(
        a, with_binds,
        "`binds` participates in arrow identity",
    );

    // It is still serialized when present.
    assert!(tagged_to_value(&with_binds).get("binds").is_some());
    assert!(tagged_to_value(&a).get("binds").is_none());
}

// ---------------------------------------------------------------------------
// 8. A round-tripped arrow is applicable without its minting runtime.
// ---------------------------------------------------------------------------

/// Build a canonical runtime with lex + Pratt parse + tree-walk eval.
fn canonical_runtime() -> handl_runtime::Runtime {
    use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::{EngineRegistry, Runtime};

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

/// Evaluate `src` in `rt` with `globals` injected.
fn eval_in(
    rt: &handl_runtime::Runtime,
    src: &str,
    globals: &std::collections::HashMap<String, Value>,
) -> Value {
    use handl_language::Source;
    let toks = rt.lex(&Source::new(src)).expect("lex");
    let ast = rt.parse(&toks).expect("parse");
    rt.eval_with_globals(&ast, globals).expect("eval")
}

/// An arrow that crossed a JSON boundary is still applicable: its body is
/// carried by the value (`body_def`), not by the runtime that minted it.
///
/// The control case is the point of the test: with `body_def` stripped the
/// same transported value can no longer be applied, which proves the
/// behaviour comes from the explicit definition rather than from any
/// ambient arena (the two runtimes are separate instances).
#[test]
fn round_tripped_arrow_is_applicable_without_its_frame() {
    use std::collections::HashMap;

    // 1. Mint in runtime A and "transport" the value as JSON.
    let a = canonical_runtime();
    let empty: HashMap<String, Value> = HashMap::new();
    let minted = eval_in(&a, "x ~> x + 1", &empty);
    assert_eq!(minted["__arrow__"], json!(true), "minted an arrow value");
    assert!(
        minted.get("body_def").is_some(),
        "the minted arrow carries its explicit body definition",
    );
    let transported: Value =
        serde_json::from_str(&minted.to_string()).expect("arrow JSON re-parses");

    // 2. Apply it in runtime B — a *fresh* runtime whose AST arena knows
    //    nothing about the minting one.
    let b = canonical_runtime();
    let mut globals = HashMap::new();
    globals.insert("f".to_string(), transported.clone());
    let applied = eval_in(&b, "f(41)", &globals);
    assert_eq!(
        as_number(&applied),
        42.0,
        "the transported arrow must apply to 41 and yield 42 (got {applied:?})",
    );
    // Equivalence with a directly-minted arrow in the *same* runtime, so the
    // transported value cannot be passing for an unrelated reason.
    let direct = eval_in(&b, "(x ~> x + 1)(41)", &empty);
    assert_eq!(
        as_number(&applied),
        as_number(&direct),
        "transported application must agree with direct application",
    );

    // 3. Control: without `body_def` the same value is not applicable —
    //    the definition is gone entirely (there is no ambient `body` index).
    let mut stripped = transported.clone();
    stripped
        .as_object_mut()
        .expect("arrow is an object")
        .remove("body_def");
    let mut globals_stripped = HashMap::new();
    globals_stripped.insert("f".to_string(), stripped);
    let stripped_result = {
        use handl_language::Source;
        let toks = b.lex(&Source::new("f(41)")).expect("lex");
        let ast = b.parse(&toks).expect("parse");
        b.eval_with_globals(&ast, &globals_stripped)
    };
    match stripped_result {
        Err(_) => {}
        Ok(v) => assert_ne!(
            as_number(&v),
            42.0,
            "without `body_def` the arrow must not evaluate to the correct \
             result — otherwise this test proves nothing",
        ),
    }
}

/// Read a numeric value out of either representation a runtime may produce
/// for a number: a bare JSON number, or a tagged `{"value": N}` record.
fn as_number(v: &Value) -> f64 {
    if let Some(n) = v.as_f64() {
        return n;
    }
    if let Some(n) = v.get("value").and_then(|inner| inner.as_f64()) {
        return n;
    }
    panic!("value is not numeric: {v:?}");
}

// ---------------------------------------------------------------------------
// 9. Referenced bindings are substituted into `body_def`, not captured.
// ---------------------------------------------------------------------------
//
// `handl_language::instantiate` replaces each serializable free variable in
// the body with a literal node, so the arrow's data *contains* the values it
// depends on. Only names that cannot be substituted — recursive-region
// binders (kept free for the fixpoint pass) and non-serializable values
// (`Resource`) — land in `binds`.
//
// These tests pin the substituted/`binds` split so a regression back to
// environment capture fails loudly.

/// A serializable referenced binding is embedded in `body_def`, not bound.
#[test]
fn referenced_serializable_binding_is_substituted_into_body_def() {
    let rt = canonical_runtime();
    let empty: std::collections::HashMap<String, Value> = std::collections::HashMap::new();

    // Body `y` reads the binding — the value 5 must live inside the arrow's
    // own data (body_def literal), and `binds` must not carry `y`.
    let arrow = eval_in(&rt, "do { y := 5; x ~> y }", &empty);
    assert_eq!(arrow["__arrow__"], json!(true), "an arrow is returned");
    assert!(
        arrow["body_def"].to_string().contains('5'),
        "the captured value 5 is embedded in `body_def`: {arrow}",
    );
    assert!(
        arrow
            .get("binds")
            .and_then(|b| b.as_object())
            .is_none_or(|b| !b.contains_key("y")),
        "a substituted binding must not appear in `binds`: {arrow}",
    );

    // And it still applies correctly.
    assert_eq!(
        as_number(&eval_in(&rt, "do { y := 5; f := x ~> y; f(9) }", &empty)),
        5.0,
    );
}

/// An unreferenced binding contributes nothing to the arrow's data.
#[test]
fn unreferenced_binding_is_not_carried() {
    let rt = canonical_runtime();
    let empty: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let arrow = eval_in(&rt, "do { y := 5; x ~> 1 }", &empty);
    assert_eq!(arrow["__arrow__"], json!(true), "an arrow is returned");
    assert!(
        arrow.get("binds").is_none()
            || !arrow["binds"]
                .as_object()
                .expect("binds")
                .contains_key("y"),
        "unreferenced bindings are not carried: {arrow}",
    );
    assert!(
        !arrow["body_def"].to_string().contains("\"lit\""),
        "a constant body needs no substitution literals: {arrow}",
    );
}

/// Soundness evidence: arrows still apply correctly in both the
/// substituted and unreferenced-body shapes, and across nesting.
#[test]
fn arrow_application_is_correct_for_both_body_shapes() {
    let rt = canonical_runtime();
    let empty: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    assert_eq!(
        as_number(&eval_in(&rt, "do { y := 5; f := x ~> 1; f(9) }", &empty)),
        1.0,
    );
    assert_eq!(
        as_number(&eval_in(&rt, "do { y := 5; f := x ~> y; f(9) }", &empty)),
        5.0,
    );
    assert_eq!(
        as_number(&eval_in(&rt, "do { f := x ~> x + 1; f(41) }", &empty)),
        42.0,
    );
    // Nested arrows: the inner body references the outer's parameter, which
    // must be substituted at inner-mint time.
    assert_eq!(
        as_number(&eval_in(
            &rt,
            "do { add := a ~> (b ~> a + b); add(40)(2) }",
            &empty
        )),
        42.0,
    );
}
