//! Tests for the `collections` package (former `plugin-lib-collections`).
//!
//! Ported from the deleted crate's coverage: the sequential containers
//! (`vec`/`list`/`queue`/`stack`/`deque`), `hash_map`/`hash_set`
//! structure semantics (dedup, get/put, ordering), and the `array`
//! functional ops — now split between pure-handl record fields
//! (`handl-packages/collections/*.hnl`) and native ops
//! (`ops::collections`).

use handl_platform::{EvalContext, EvalError, Module};
use std::sync::Arc;

use handl_plugin_lib_stdlib::ops::collections::CollectionsModule;
use handl_plugin_lib_stdlib::{load_hnl_file, module_ref};

/// The `array` package is file-driven (loaded from
/// `collections/array/index.hnl`, not a table `ModuleSpec`).
fn array_module() -> Arc<handl_plugin_lib_handl::HandlModule> {
    load_hnl_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../handl-packages/collections/array/index.hnl"
    ))
    .expect("array module")
}

/// Call a native `array.*` op (`push`/`range`/arrow-taking members) through
/// the collections `OpRegistry`.
fn call_op(op: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = handl_plugin_lib_stdlib::ops::collections::collections_op_registry()
        .get(op)
        .unwrap_or_else(|| panic!("missing op {op}"));
    let mut ctx = EvalContext::new();
    norm(&f.call(args, &mut ctx).expect("call succeeds"))
}

/// Normalize a JSON value for comparison: handl-produced numbers are
/// integers while the native ops produce f64, so numeric leaves are
/// compared as f64.
fn norm(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Number(n) => serde_json::json!(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(norm).collect()),
        serde_json::Value::Object(o) => {
            serde_json::Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect())
        }
        other => other.clone(),
    }
}

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    norm(&func.call(args, &mut ctx).expect("call succeeds"))
}

fn call_err(
    module: &dyn Module,
    name: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, EvalError> {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

fn n(v: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(v))
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

fn arr(values: &[serde_json::Value]) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Array(values.to_vec()))
}

fn obj(entries: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v.clone());
    }
    serde_json::Value::from(serde_json::Value::Object(map))
}

fn collections() -> CollectionsModule {
    CollectionsModule::new(Arc::new(handl_plugin_lib_stdlib::DEFAULT_RESOLVER.clone()))
}

// ---------------------------------------------------------------------------
// Aggregate dispatch — `vec.new` through the `collections` module, plus the
// individual module names registered in the package table.
// ---------------------------------------------------------------------------

#[test]
fn aggregate_dispatches_dotted_names() {
    let c = collections();
    assert_eq!(call(&c, "vec.new", &[]), arr(&[]));
    assert_eq!(call(&c, "array.len", &[arr(&[n(1.0), n(2.0)])]), n(2.0));
    assert_eq!(call(&c, "hash_map.new", &[]), obj(&[]));
    // Unknown prefix resolves to None.
    assert!(c.function("bogus.new").is_none());
}

// ---------------------------------------------------------------------------
// vec (back-ended dynamic array)
// ---------------------------------------------------------------------------

#[test]
fn vec_push_pop_peek() {
    let m = module_ref("vec").expect("vec module");
    let v = call(m, "new", &[]);
    assert_eq!(v, arr(&[]));
    let v = call(m, "push", &[v, n(1.0)]);
    let v = call(m, "push", &[v, n(2.0)]);
    assert_eq!(v, arr(&[n(1.0), n(2.0)]));
    assert_eq!(call(m, "len", &[v.clone()]), n(2.0));
    assert_eq!(call(m, "is_empty", &[v.clone()]), serde_json::json!(false));
    assert_eq!(call(m, "peek", &[v.clone()]), n(2.0));
    assert_eq!(call(m, "pop", &[v.clone()]), arr(&[n(1.0)]));
    assert_eq!(call(m, "to_array", &[v.clone()]), arr(&[n(1.0), n(2.0)]));
    assert_eq!(call(m, "from", &[arr(&[n(9.0)])]), arr(&[n(9.0)]));
}

#[test]
fn vec_pop_empty_errors() {
    let m = module_ref("vec").expect("vec module");
    let err = call_err(m, "pop", &[arr(&[])]).unwrap_err();
    assert!(err.to_string().contains("empty"), "unexpected: {err}");
    assert!(call_err(m, "peek", &[arr(&[])]).is_err());
}

// ---------------------------------------------------------------------------
// list (front-ended)
// ---------------------------------------------------------------------------

#[test]
fn list_push_pop_peek_front() {
    let m = module_ref("list").expect("list module");
    let v = call(m, "new", &[]);
    let v = call(m, "push", &[v, n(1.0)]);
    let v = call(m, "push", &[v, n(2.0)]);
    assert_eq!(v, arr(&[n(2.0), n(1.0)]));
    assert_eq!(call(m, "peek", &[v.clone()]), n(2.0));
    assert_eq!(call(m, "pop", &[v.clone()]), arr(&[n(1.0)]));
}

// ---------------------------------------------------------------------------
// queue (FIFO)
// ---------------------------------------------------------------------------

#[test]
fn queue_fifo_order() {
    let m = module_ref("queue").expect("queue module");
    let q = call(m, "new", &[]);
    let q = call(m, "enqueue", &[q, n(1.0)]);
    let q = call(m, "enqueue", &[q, n(2.0)]);
    assert_eq!(call(m, "peek", &[q.clone()]), n(1.0));
    assert_eq!(call(m, "dequeue", &[q.clone()]), arr(&[n(2.0)]));
    assert_eq!(call(m, "len", &[q]), n(2.0));
}

// ---------------------------------------------------------------------------
// stack (LIFO)
// ---------------------------------------------------------------------------

#[test]
fn stack_lifo_order() {
    let m = module_ref("stack").expect("stack module");
    let st = call(m, "new", &[]);
    let st = call(m, "push", &[st, n(1.0)]);
    let st = call(m, "push", &[st, n(2.0)]);
    assert_eq!(call(m, "peek", &[st.clone()]), n(2.0));
    assert_eq!(call(m, "pop", &[st.clone()]), arr(&[n(1.0)]));
}

// ---------------------------------------------------------------------------
// deque (double-ended)
// ---------------------------------------------------------------------------

#[test]
fn deque_both_ends() {
    let m = module_ref("deque").expect("deque module");
    let d = call(m, "new", &[]);
    let d = call(m, "push_back", &[d, n(2.0)]);
    let d = call(m, "push_front", &[d, n(1.0)]);
    let d = call(m, "push_back", &[d, n(3.0)]);
    assert_eq!(d, arr(&[n(1.0), n(2.0), n(3.0)]));
    assert_eq!(call(m, "peek_front", &[d.clone()]), n(1.0));
    assert_eq!(call(m, "peek_back", &[d.clone()]), n(3.0));
    assert_eq!(call(m, "pop_front", &[d.clone()]), arr(&[n(2.0), n(3.0)]));
    assert_eq!(call(m, "pop_back", &[d]), arr(&[n(1.0), n(2.0)]));
}

// ---------------------------------------------------------------------------
// hash_map (native — object key iteration)
// ---------------------------------------------------------------------------

#[test]
fn hash_map_get_set_has_remove() {
    let m = module_ref("hash_map").expect("hash_map module");
    let map = call(m, "new", &[]);
    let map = call(m, "set", &[map, s("a"), n(1.0)]);
    let map = call(m, "set", &[map, s("b"), n(2.0)]);
    // `set` replaces an existing key.
    let map = call(m, "set", &[map, s("a"), n(10.0)]);
    assert_eq!(call(m, "get", &[map.clone(), s("a")]), n(10.0));
    assert_eq!(
        call(m, "get", &[map.clone(), s("missing")]),
        serde_json::Value::Null
    );
    assert_eq!(
        call(m, "get", &[map.clone(), s("missing"), n(42.0)]),
        n(42.0)
    );
    assert_eq!(
        call(m, "has", &[map.clone(), s("b")]),
        serde_json::json!(true)
    );
    assert_eq!(
        call(m, "has_key", &[map.clone(), s("z")]),
        serde_json::json!(false)
    );
    assert_eq!(call(m, "len", &[map.clone()]), n(2.0));
    let map = call(m, "remove", &[map, s("a")]);
    assert_eq!(
        call(m, "has", &[map.clone(), s("a")]),
        serde_json::json!(false)
    );
    assert_eq!(call(m, "size", &[map]), n(1.0));
}

#[test]
fn hash_map_keys_values_entries() {
    let m = module_ref("hash_map").expect("hash_map module");
    let map = obj(&[("a", n(1.0)), ("b", n(2.0))]);
    let mut keys = call(m, "keys", &[map.clone()]);
    keys.as_array_mut()
        .unwrap()
        .sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
    assert_eq!(keys, arr(&[s("a"), s("b")]));
    let mut values = call(m, "values", &[map.clone()]);
    values
        .as_array_mut()
        .unwrap()
        .sort_by(|a, b| a.as_f64().partial_cmp(&b.as_f64()).unwrap());
    assert_eq!(values, arr(&[n(1.0), n(2.0)]));
    let entries = call(m, "entries", &[map]);
    assert_eq!(entries.as_array().unwrap().len(), 2);
}

#[test]
fn hash_map_paths_and_merges() {
    let m = module_ref("hash_map").expect("hash_map module");
    let nested = obj(&[("a", obj(&[("b", obj(&[("c", n(7.0))]))]))]);
    assert_eq!(call(m, "deep_get", &[nested.clone(), s("a.b.c")]), n(7.0));
    assert_eq!(
        call(m, "path_or", &[nested.clone(), s("a.b.x"), n(0.0)]),
        n(0.0)
    );
    let set = call(m, "set_path", &[obj(&[]), s("x.y"), n(5.0)]);
    assert_eq!(call(m, "deep_get", &[set, s("x.y")]), n(5.0));

    let a = obj(&[("k", n(1.0)), ("shared", n(1.0))]);
    let b = obj(&[("shared", n(9.0))]);
    let merged = call(m, "merge", &[a, b]);
    assert_eq!(call(m, "get", &[merged, s("shared")]), n(9.0));
    let dm = call(
        m,
        "deep_merge",
        &[
            obj(&[("o", obj(&[("x", n(1.0))]))]),
            obj(&[("o", obj(&[("y", n(2.0))]))]),
        ],
    );
    assert_eq!(call(m, "deep_get", &[dm.clone(), s("o.x")]), n(1.0));
    assert_eq!(call(m, "deep_get", &[dm, s("o.y")]), n(2.0));
}

#[test]
fn hash_map_pick_omit_invert_from_entries() {
    let m = module_ref("hash_map").expect("hash_map module");
    let map = obj(&[("a", n(1.0)), ("b", n(2.0)), ("c", n(3.0))]);
    let picked = call(m, "pick", &[map.clone(), arr(&[s("a"), s("c")])]);
    assert_eq!(call(m, "len", &[picked]), n(2.0));
    let omitted = call(m, "omit", &[map.clone(), arr(&[s("b")])]);
    assert_eq!(call(m, "has", &[omitted, s("b")]), serde_json::json!(false));
    let inverted = call(m, "invert", &[obj(&[("k", s("v"))])]);
    assert_eq!(call(m, "get", &[inverted, s("v")]), s("k"));
    let built = call(
        m,
        "from_entries",
        &[arr(&[arr(&[s("x"), n(1.0)]), arr(&[s("y"), n(2.0)])])],
    );
    assert_eq!(call(m, "get", &[built, s("y")]), n(2.0));
    assert_eq!(
        call(
            m,
            "get_or",
            &[obj(&[("a", serde_json::Value::Null)]), s("a"), n(3.0)]
        ),
        n(3.0)
    );
    // clone_deep is a deep copy (identity over values).
    let v = obj(&[("nested", arr(&[n(1.0)]))]);
    assert_eq!(call(m, "clone_deep", &[v.clone()]), v);
}

// ---------------------------------------------------------------------------
// hash_set (native — serialized-key dedup)
// ---------------------------------------------------------------------------

#[test]
fn hash_set_dedup_and_membership() {
    let m = module_ref("hash_set").expect("hash_set module");
    // `from` deduplicates preserving first-occurrence order.
    let st = call(m, "from", &[arr(&[n(1.0), n(2.0), n(1.0), n(3.0), n(2.0)])]);
    assert_eq!(st, arr(&[n(1.0), n(2.0), n(3.0)]));
    assert_eq!(call(m, "len", &[st.clone()]), n(3.0));
    assert_eq!(
        call(m, "has", &[st.clone(), n(2.0)]),
        serde_json::json!(true)
    );
    // `add` is idempotent.
    assert_eq!(call(m, "add", &[st.clone(), n(2.0)]), st);
    assert_eq!(
        call(m, "add", &[st.clone(), n(4.0)]),
        arr(&[n(1.0), n(2.0), n(3.0), n(4.0)])
    );
    assert_eq!(
        call(m, "remove", &[st.clone(), n(2.0)]),
        arr(&[n(1.0), n(3.0)])
    );
    assert_eq!(call(m, "to_array", &[st]), arr(&[n(1.0), n(2.0), n(3.0)]));
}

#[test]
fn hash_set_algebra() {
    let m = module_ref("hash_set").expect("hash_set module");
    let a = arr(&[n(1.0), n(2.0), n(3.0)]);
    let b = arr(&[n(2.0), n(3.0), n(4.0)]);
    assert_eq!(
        call(m, "union", &[a.clone(), b.clone()]),
        arr(&[n(1.0), n(2.0), n(3.0), n(4.0)])
    );
    assert_eq!(
        call(m, "intersection", &[a.clone(), b.clone()]),
        arr(&[n(2.0), n(3.0)])
    );
    assert_eq!(
        call(m, "difference", &[a.clone(), b.clone()]),
        arr(&[n(1.0)])
    );
    assert_eq!(
        call(m, "symmetric_difference", &[a.clone(), b.clone()]),
        arr(&[n(1.0), n(4.0)])
    );
    assert_eq!(
        call(m, "without", &[a.clone(), n(2.0), n(99.0)]),
        arr(&[n(1.0), n(3.0)])
    );
    assert_eq!(
        call(m, "unique", &[arr(&[s("x"), s("y"), s("x")])]),
        arr(&[s("x"), s("y")])
    );
}

// ---------------------------------------------------------------------------
// array — pure-handl ops
// ---------------------------------------------------------------------------

#[test]
fn array_basics() {
    let m = array_module();
    let a = arr(&[n(1.0), n(2.0), n(3.0)]);
    assert_eq!(call(m.as_ref(), "len", &[a.clone()]), n(3.0));
    assert_eq!(call(m.as_ref(), "is_empty", &[a.clone()]), serde_json::json!(false));
    assert_eq!(call(m.as_ref(), "is_empty", &[arr(&[])]), serde_json::json!(true));
    assert_eq!(
        call(m.as_ref(), "reverse", &[a.clone()]),
        arr(&[n(3.0), n(2.0), n(1.0)])
    );
    assert_eq!(
        call(m.as_ref(), "concat", &[a.clone(), arr(&[n(4.0)])]),
        arr(&[n(1.0), n(2.0), n(3.0), n(4.0)])
    );
    assert_eq!(
        call(m.as_ref(), "contains", &[a.clone(), n(2.0)]),
        serde_json::json!(true)
    );
    assert_eq!(
        call(m.as_ref(), "contains", &[a.clone(), n(9.0)]),
        serde_json::json!(false)
    );
    assert_eq!(call(m.as_ref(), "index_of", &[a.clone(), n(3.0)]), n(2.0));
    assert_eq!(call(m.as_ref(), "index_of", &[a.clone(), n(9.0)]), n(-1.0));
    assert_eq!(call(m.as_ref(), "first", &[a.clone()]), n(1.0));
    assert_eq!(call(m.as_ref(), "last", &[a.clone()]), n(3.0));
    assert_eq!(call(m.as_ref(), "at", &[a.clone(), n(1.0)]), n(2.0));
    assert_eq!(call(m.as_ref(), "at", &[a.clone(), n(9.0)]), serde_json::Value::Null);
    assert_eq!(call(m.as_ref(), "first", &[arr(&[])]), serde_json::Value::Null);
    assert_eq!(call(m.as_ref(), "last", &[arr(&[])]), serde_json::Value::Null);
}

#[test]
fn array_push_pop_shift_slice() {
    let m = array_module();
    let a = arr(&[n(1.0), n(2.0), n(3.0)]);
    // `push` is variadic (rest param).
    assert_eq!(
        call_op("array.push", &[a.clone(), n(4.0), n(5.0)]),
        arr(&[n(1.0), n(2.0), n(3.0), n(4.0), n(5.0)])
    );
    assert_eq!(call(m.as_ref(), "pop", &[a.clone()]), arr(&[n(1.0), n(2.0)]));
    assert_eq!(call(m.as_ref(), "pop", &[arr(&[])]), arr(&[]));
    assert_eq!(call(m.as_ref(), "shift", &[a.clone()]), arr(&[n(2.0), n(3.0)]));
    assert_eq!(call(m.as_ref(), "shift", &[arr(&[])]), arr(&[]));
    assert_eq!(
        call(m.as_ref(), "prepend", &[a.clone(), n(0.0)]),
        arr(&[n(0.0), n(1.0), n(2.0), n(3.0)])
    );
    assert_eq!(
        call(m.as_ref(), "slice", &[a.clone(), n(0.0), n(2.0)]),
        arr(&[n(1.0), n(2.0)])
    );
    // end is clamped to len; start clamped to end.
    assert_eq!(
        call(m.as_ref(), "slice", &[a.clone(), n(1.0), n(99.0)]),
        arr(&[n(2.0), n(3.0)])
    );
}

#[test]
fn array_take_drop_fill_range() {
    let m = array_module();
    let a = arr(&[n(1.0), n(2.0), n(3.0), n(4.0)]);
    assert_eq!(
        call(m.as_ref(), "take", &[a.clone(), n(2.0)]),
        arr(&[n(1.0), n(2.0)])
    );
    assert_eq!(call(m.as_ref(), "take", &[a.clone(), n(99.0)]), a);
    assert_eq!(
        call(m.as_ref(), "drop", &[a.clone(), n(2.0)]),
        arr(&[n(3.0), n(4.0)])
    );
    assert_eq!(call(m.as_ref(), "drop", &[a.clone(), n(99.0)]), arr(&[]));
    assert_eq!(
        call(m.as_ref(), "fill", &[n(3.0), s("x")]),
        arr(&[s("x"), s("x"), s("x")])
    );
    // range with default and explicit step.
    assert_eq!(
        call_op("array.range", &[n(0.0), n(4.0)]),
        arr(&[n(0.0), n(1.0), n(2.0), n(3.0)])
    );
    assert_eq!(
        call_op("array.range", &[n(0.0), n(6.0), n(2.0)]),
        arr(&[n(0.0), n(2.0), n(4.0)])
    );
    assert_eq!(
        call_op("array.range", &[n(5.0), n(0.0), n(-2.0)]),
        arr(&[n(5.0), n(3.0), n(1.0)])
    );
}

#[test]
fn array_flatten_family() {
    let m = array_module();
    let nested = arr(&[arr(&[n(1.0), n(2.0)]), n(3.0), arr(&[arr(&[n(4.0)])])]);
    assert_eq!(
        call(m.as_ref(), "flatten", &[nested.clone()]),
        arr(&[n(1.0), n(2.0), n(3.0), arr(&[n(4.0)])])
    );
    assert_eq!(
        call(m.as_ref(), "deep_flatten", &[nested]),
        arr(&[n(1.0), n(2.0), n(3.0), n(4.0)])
    );
    assert_eq!(call(m.as_ref(), "flatten", &[arr(&[])]), arr(&[]));
}

#[test]
fn array_pairs_and_windows() {
    let m = array_module();
    let a = arr(&[s("a"), s("b")]);
    assert_eq!(
        call(m.as_ref(), "enumerate", &[a]),
        arr(&[arr(&[n(0.0), s("a")]), arr(&[n(1.0), s("b")])])
    );
    let pairs = arr(&[arr(&[n(1.0), s("x")]), arr(&[n(2.0), s("y")])]);
    assert_eq!(
        call(m.as_ref(), "unzip", &[pairs.clone()]),
        arr(&[arr(&[n(1.0), n(2.0)]), arr(&[s("x"), s("y")])])
    );
    assert_eq!(
        call(
            m.as_ref(),
            "zip",
            &[arr(&[n(1.0), n(2.0), n(3.0)]), arr(&[s("a"), s("b")])]
        ),
        arr(&[arr(&[n(1.0), s("a")]), arr(&[n(2.0), s("b")])])
    );
    let xs = arr(&[n(1.0), n(2.0), n(3.0), n(4.0)]);
    assert_eq!(
        call(m.as_ref(), "windows", &[xs.clone(), n(2.0)]),
        arr(&[
            arr(&[n(1.0), n(2.0)]),
            arr(&[n(2.0), n(3.0)]),
            arr(&[n(3.0), n(4.0)])
        ])
    );
    assert_eq!(call(m.as_ref(), "windows", &[xs.clone(), n(9.0)]), arr(&[]));
    assert_eq!(
        call(m.as_ref(), "chunk", &[xs, n(3.0)]),
        arr(&[arr(&[n(1.0), n(2.0), n(3.0)]), arr(&[n(4.0)])])
    );
}

#[test]
fn array_dedupe_compact_transpose_insert_remove() {
    let m = array_module();
    // `dedupe` removes only *consecutive* duplicates.
    assert_eq!(
        call(m.as_ref(), "dedupe", &[arr(&[n(1.0), n(1.0), n(2.0), n(1.0)])]),
        arr(&[n(1.0), n(2.0), n(1.0)])
    );
    assert_eq!(
        call(
            m.as_ref(),
            "compact",
            &[arr(&[n(1.0), serde_json::Value::Null, n(2.0)])]
        ),
        arr(&[n(1.0), n(2.0)])
    );
    assert_eq!(
        call(
            m.as_ref(),
            "transpose",
            &[arr(&[arr(&[n(1.0), n(2.0)]), arr(&[n(3.0), n(4.0)])])]
        ),
        arr(&[arr(&[n(1.0), n(3.0)]), arr(&[n(2.0), n(4.0)])])
    );
    assert_eq!(
        call(m.as_ref(), "insert_at", &[arr(&[n(1.0), n(3.0)]), n(1.0), n(2.0)]),
        arr(&[n(1.0), n(2.0), n(3.0)])
    );
    assert_eq!(
        call(m.as_ref(), "remove_at", &[arr(&[n(1.0), n(2.0), n(3.0)]), n(1.0)]),
        arr(&[n(1.0), n(3.0)])
    );
    assert_eq!(
        call(m.as_ref(), "remove_at", &[arr(&[n(1.0)]), n(9.0)]),
        arr(&[n(1.0)])
    );
}

#[test]
fn array_intersperse_intercalate_pluck() {
    let m = array_module();
    assert_eq!(
        call(m.as_ref(), "intersperse", &[arr(&[n(1.0), n(2.0), n(3.0)]), n(0.0)]),
        arr(&[n(1.0), n(0.0), n(2.0), n(0.0), n(3.0)])
    );
    assert_eq!(
        call(
            m.as_ref(),
            "intercalate",
            &[
                arr(&[arr(&[n(1.0)]), arr(&[n(2.0), n(3.0)])]),
                arr(&[n(0.0)])
            ]
        ),
        arr(&[n(1.0), n(0.0), n(2.0), n(3.0)])
    );
    let rows = arr(&[obj(&[("k", s("a"))]), obj(&[("k", s("b"))]), n(5.0)]);
    assert_eq!(
        call(m.as_ref(), "pluck", &[rows, s("k")]),
        arr(&[s("a"), s("b"), serde_json::Value::Null])
    );
}

// ---------------------------------------------------------------------------
// array — the arrow-taking ops remain native and resolve under the same
// names (calling them requires an EvalContext with eval machinery, so here
// we only assert they are registered and dispatchable).
// ---------------------------------------------------------------------------

#[test]
fn array_arrow_ops_registered() {
    let registry =
        handl_plugin_lib_stdlib::ops::collections::collections_op_registry();
    for name in [
        "take_while",
        "drop_while",
        "scan",
        "foldr",
        "find_index",
        "find_last",
        "find_last_index",
    ] {
        assert!(
            registry.contains(&format!("array.{name}")),
            "missing array.{name}"
        );
    }
}
