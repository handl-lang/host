//! ADR-0106 §6/§8 — arrow serializability conformance tests (Phase 5).
//!
//! ADR-0106 §3/§6/§8 require arrows to be *objects*: serializable,
//! inspectable, with **no closures, no lexical environments, no hidden
//! captured state** (`A ~> B` is a value you can store, transmit, and cache).
//! The tree today does **not** fully meet that: an arrow value
//! (`TaggedValue::Arrow { params, rest, body, env, home, array_pattern }`,
//! `crates/handl-value/src/lib.rs:345`) still carries a captured `env`
//! (`Rc<FxHashMap<Rc<str>, TaggedValue>>`, `:353`) and a `body` AST index
//! (`:352`) whose meaning lives in a **process-local** home-frame table
//! (`HOME_FRAMES`, `crates/plugin-engine-exec-tree-walk/src/lib.rs:134`).
//!
//! These tests do **not** try to remove that substrate (the closure/scope
//! machinery is load-bearing — ADR-0106 Phase 5 scope). They pin down what is
//! *currently true and checkable through public APIs* so that later kernel
//! work has a regression fence:
//!
//! 1. the JSON encoding of an arrow uses exactly the documented key universe
//!    (`__arrow__`, `params`, `body`, `env`, `rest`, `__home__`,
//!    `array_pattern`) — silent additions fail loudly;
//! 2. an arrow with an empty `env` round-trips JSON -> value -> JSON unchanged;
//! 3. an arrow whose `env` captures a nested arrow with a `home` id round-trips
//!    **exactly** at the value level (the `__home__` id is serialized data);
//! 4. an arrow whose `env` captures a `Resource` round-trips **lossily** (the
//!    finalizer is dropped — see the test's comment for the precise result);
//! 5. a composed arrow (`a >>> b`, represented as the `Seq` term) has no hidden
//!    state beyond its two component terms.
//!
//! ## Not yet checkable through public APIs
//!
//! TODO(ADR-0106): the invariant that actually matters — *an arrow's `body`
//! index is meaningless without the engine that minted it* — is **not**
//! observable through the public value API. `tagged_to_value` happily emits
//! `"body": <idx>`, and a decoded arrow compares equal to the live one, yet
//! the AST that index resolves against lives only in
//! `HOME_FRAMES[id].ast` (`crates/plugin-engine-exec-tree-walk/src/lib.rs:155`)
//! and is never serialized. There is no public accessor that pairs an arrow
//! value with its frame, nor a JSON encoding that embeds the referenced AST,
//! so a round-tripped *foreign-frame* arrow cannot be applied — the assertion
//! "a round-tripped arrow is applicable" is unverifiable here and is left as a
//! TODO rather than invented. Likewise, the shared scope stacks / captured
//! environments of the bytecode (`plugin-engine-exec-bytecode/src/vm.rs:133`)
//! and cranelift (`plugin-engine-exec-cranelift/src/lib.rs:613`) engines are
//! `pub(crate)` and not exposed for inspection.

use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_value::{resource, tagged_to_value, TaggedValue};
use plugin_engine::TermData;
use rustc_hash::FxHashMap;
use serde_json::{json, Value};

/// The documented JSON key universe for an arrow value (ADR-0106 §6; the
/// `TaggedValue::Arrow` variant fields of `handl-value/src/lib.rs:345`).
const DOCUMENTED_ARROW_KEYS: &[&str] = &[
    "__arrow__",
    "params",
    "body",
    "env",
    "rest",
    "__home__",
    "array_pattern",
];

/// Round-trip a JSON value through the tagged-value representation.
fn round_trip(v: Value) -> Value {
    let t: TaggedValue = v.into();
    tagged_to_value(&t)
}

/// A bare arrow (`x ~> x`) with an empty env — the value the tree-walk engine
/// mints for `x ~> x` (see `proc_notation.rs`).
fn bare_arrow_json() -> Value {
    json!({ "__arrow__": true, "params": ["x"], "body": 0, "env": {} })
}

// ---------------------------------------------------------------------------
// 1. The JSON shape is pinned to the documented key universe.
// ---------------------------------------------------------------------------

/// A bare arrow emits **exactly** the four mandatory keys — no more. New keys
/// added to the encoder fail this test loudly (pin the shape).
#[test]
fn bare_arrow_json_has_exactly_the_mandatory_keys() {
    let out = round_trip(bare_arrow_json());
    let obj = out.as_object().expect("arrow encodes as a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["__arrow__", "body", "env", "params"],
        "bare arrow key set changed; if a new field is intentional, extend \
         DOCUMENTED_ARROW_KEYS and the ADR-0106 Phase 5 conformance note",
    );
}

/// The optional keys (`rest`, `__home__`, `array_pattern`) appear only when
/// set, and every emitted key is inside the documented universe.
#[test]
fn arrow_json_keys_are_a_subset_of_the_documented_universe() {
    // Full arrow: every optional field present.
    let full = json!({
        "__arrow__": true,
        "params": ["a"],
        "rest": "more",
        "body": 3,
        "env": {},
        "__home__": 9,
        "array_pattern": true,
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
    // The variant field `home` is spelled `__home__` on the JSON side (not
    // `home`); the value's `rest`/`array_pattern` map to identically-named keys.
    assert!(obj.contains_key("__home__"));
    assert!(!obj.contains_key("home"));
}

// ---------------------------------------------------------------------------
// 2. Empty-env arrow round-trips JSON -> value -> JSON unchanged.
// ---------------------------------------------------------------------------

#[test]
fn empty_env_arrow_round_trips_unchanged() {
    for arrow in [
        bare_arrow_json(),
        json!({ "__arrow__": true, "params": ["x", "y"], "body": 7, "env": {} }),
    ] {
        assert_eq!(round_trip(arrow.clone()), arrow);
    }
}

/// Round-tripping is stable across two passes (idempotent) for a bare arrow.
#[test]
fn empty_env_arrow_round_trip_is_idempotent() {
    let once = round_trip(bare_arrow_json());
    let twice = round_trip(once.clone());
    assert_eq!(once, twice);
}

// ---------------------------------------------------------------------------
// 3. Nested arrow captured in `env`, with a `home` id — round-trips exactly.
// ---------------------------------------------------------------------------

/// An arrow whose `env` captures a *nested arrow that itself has a `home` id*
/// round-trips **exactly** at the value level: the `__home__` id is explicit
/// serialized data (`tagged_to_value`, `handl-value/src/lib.rs:750`), so the
/// decoded value compares equal to the live one.
///
/// Precise result recorded: the id survives; but the *frame it names*
/// (`HOME_FRAMES[id]` — AST + Language + modules + globals) does **not**, so
/// although the value round-trips the captured arrow is not applicable after
/// the crossing unless that frame still exists in-process. That residual is
/// the ADR-0106 §6 violation and is noted here, not asserted (see the file
/// header's TODO(ADR-0106)).
#[test]
fn env_captured_arrow_with_home_id_round_trips_exactly() {
    let outer = json!({
        "__arrow__": true,
        "params": ["x"],
        "body": 0,
        "env": {
            "inner": {
                "__arrow__": true,
                "params": ["y"],
                "body": 4,
                "env": {},
                "__home__": 7,
            }
        },
    });
    let out = round_trip(outer.clone());
    assert_eq!(out, outer, "nested homed arrow must round-trip byte-for-byte");
    assert_eq!(
        out["env"]["inner"]["__home__"],
        json!(7),
        "the nested arrow's `home` id is preserved on the JSON side",
    );
}


// ---------------------------------------------------------------------------
// 4. Resource captured in `env` — round-trips *lossily*.
// ---------------------------------------------------------------------------

/// Set true by the finalizer below.
static FINALIZED: AtomicBool = AtomicBool::new(false);

unsafe fn mark_finalized(_ptr: *mut c_void) {
    FINALIZED.store(true, Ordering::SeqCst);
}

/// An arrow whose `env` captures a `Resource` (a foreign RAII handle)
/// round-trips **lossily**.
///
/// Precise result recorded by this test:
/// - the JSON encoding is the debug-only `{"__resource__": {"ptr": N}}`
///   (`handl-value/src/lib.rs:758`) — it carries the pointer but **drops the
///   finalizer** (a function pointer, not serializable);
/// - the decoded `Resource` has `finalizer = None` (see the decoder at
///   `handl-value/src/lib.rs:611`), so dropping the round-tripped arrow does
///   **not** run the original finalizer — the resource leaks. This is exactly
///   the "hidden captured state that is not serializable" ADR-0106 §6 objects
///   to.
#[test]
fn env_captured_resource_round_trips_lossily() {
    // Build a *live* resource carrying a finalizer, then nest it in an arrow's
    // env. `ptr = 1` is never dereferenced by ForeignResource (opaque handle).
    let live = unsafe { resource(1 as *mut c_void, Some(mark_finalized)) };
    let mut env: FxHashMap<Rc<str>, TaggedValue> = FxHashMap::default();
    env.insert(Rc::from("handle"), live);
    let arrow = TaggedValue::Arrow {
        params: Rc::from(vec!["x".to_string()]),
        rest: None,
        body: 0,
        env: Rc::new(env),
        home: None,
        array_pattern: false,
    };

    // Serialize: the resource becomes a debug-only pointer record, no finalizer.
    let encoded = tagged_to_value(&arrow);
    assert_eq!(
        encoded["env"]["handle"],
        json!({ "__resource__": { "ptr": 1 } }),
        "resource encodes as a debug-only pointer record",
    );

    // Round-trip the *encoded* form: the decoded resource has no finalizer.
    FINALIZED.store(false, Ordering::SeqCst);
    let decoded: TaggedValue = encoded.clone().into();
    match &decoded {
        TaggedValue::Arrow { env, .. } => {
            let handle = env.get("handle").expect("decoded env keeps `handle`");
            assert!(
                matches!(handle, TaggedValue::Resource(_)),
                "decoded env value is a Resource",
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
/// "fst": <a>, "snd": <b>}` — a plain serializable tagged record with only its
/// two components; the encoder adds nothing else. (At the v0.1 runtime, `>>>`
/// surface syntax is actually *application*, not composition — see
/// `handl-kit-data-canonical/data/types.yaml:2158-2166` — so no composed-arrow
/// *value* with hidden state is produced today.
///
/// TODO(ADR-0106): when `>>>` lowers to genuine arrow composition (ADR-0106
/// §3), add a test that the composed arrow value (if any) carries no captured
/// state beyond its two component arrows.
#[test]
fn composed_seq_term_has_only_component_fields() {
    let seq = TermData::seq(
        TermData::arr(json!({"__arrow__": true, "params": ["x"], "body": 0, "env": {}})),
        TermData::arr(json!({"__arrow__": true, "params": ["y"], "body": 0, "env": {}})),
    );
    let obj = seq
        .0
        .as_object()
        .expect("Seq term encodes as a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["__tag__", "fst", "snd"],
        "the Seq composition record must carry only its two components",
    );
    // The components are themselves the (serializable) arrow records, each
    // wrapped as the `value` of an `Arr` term.
    assert_eq!(obj["fst"]["value"]["__arrow__"], json!(true));
    assert_eq!(obj["snd"]["value"]["__arrow__"], json!(true));
    // And the whole record round-trips unchanged.
    assert_eq!(round_trip(seq.0.clone()), seq.0);
}

// ---------------------------------------------------------------------------
// 6. The arrow the engine actually mints has the documented shape.
// ---------------------------------------------------------------------------

/// The tree-walk engine, asked to evaluate `x ~> x`, returns an arrow whose
/// JSON keys are all inside the documented universe and whose mandatory keys
/// carry the expected values (`params == ["x"]`, empty `env`). This ties the
/// value-level shape (above) to the engine that mints arrows.
#[test]
fn engine_arrow_value_uses_documented_shape() {
    use handl_language::Source;
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
    let rt = Runtime::from_registry(kit.lang().clone(), &registry, &config)
        .expect("runtime binds engines");

    let arrow = rt.run(&Source::new("x ~> x")).expect("x ~> x evaluates");

    assert_eq!(arrow["__arrow__"], json!(true), "result is an arrow value");
    assert_eq!(arrow["params"], json!(["x"]));
    assert_eq!(arrow["env"], json!({}), "a minted arrow's env is empty here");
    assert!(arrow["body"].is_u64(), "arrow carries a numeric body index");

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



