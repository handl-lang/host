//! Regression test: `ModuleLoader` caches evaluated module bindings as a
//! shared [`TaggedValue`] record, not an expanded `serde_json::Value` tree.
//!
//! The previous `Value`-keyed cache pinned a fully-expanded JSON document
//! per module — `Rc`-shared arrow `binds` maps (ADR-0106 rec regions) were
//! duplicated at every arrow — which dominated resident memory on
//! multi-file `hnl test` runs. The tagged cache keeps the eval-time value
//! graph intact: `resolve_import_tagged` hands out `Rc` clones of the one
//! cached record.
//!
//! Covered here:
//! - repeated `resolve_import_tagged` returns the *same* `Rc` (no deep
//!   copy per import);
//! - `resolve_import` (the `Value` boundary) still produces the export
//!   record;
//! - an imported arrow applies correctly through a `Runtime` — including
//!   on a second (cache-hit) import — so sharing does not break closure
//!   semantics across file boundaries.

use std::path::PathBuf;
use std::rc::Rc;

use handl_language::{Source, Value};
use handl_plugin_engine_exec::ImportResolver;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{ModuleLoader, Runtime};
use handl_value::TaggedValue;
use serde_json::json;

/// A unique temp directory for this test process's module fixtures.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hnl-modcache-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Build a `ModuleLoader` rooted at `module_root` with real engines, plus a
/// `Runtime` whose import resolver is the same loader — the wiring
/// `kit-canonical` performs for `import "<file>"`.
fn loader_and_runtime(module_root: PathBuf) -> (Rc<ModuleLoader>, Runtime) {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    let lang_rc = Rc::new(kit.lang().clone());
    let loader = Rc::new(ModuleLoader::new(
        Rc::clone(&lang_rc),
        Box::new(DataLexerEngine::new()),
        Box::new(PrattEngine::new()),
        Some(module_root),
    ));
    let mut loader_eval =
        TreeWalkEngine::new().with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
    // Ambient language globals (`true`/`false`) must be in scope inside
    // imported modules too — same as the kit wiring.
    for (name, val) in &kit.lang().globals {
        loader_eval = loader_eval.with_global(name, val.clone());
    }
    loader.set_eval_engine(Box::new(loader_eval));

    let runtime = Runtime::new(kit.lang().clone())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(
            TreeWalkEngine::new()
                .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>),
        ))
        .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
    (loader, runtime)
}

#[test]
fn tagged_cache_returns_shared_record() {
    let dir = scratch_dir("shared");
    std::fs::write(
        dir.join("m.hnl"),
        "export inc := (x) ~> x + 1\nexport forty_two := 42\n",
    )
    .expect("write module");

    let (loader, _rt) = loader_and_runtime(dir.clone());

    let first = loader
        .resolve_import_tagged("m.hnl")
        .expect("first tagged load");
    let second = loader
        .resolve_import_tagged("m.hnl")
        .expect("second tagged load");

    // The cache hit must return the same shared record — an `Rc` bump,
    // not a deep `TaggedValue`/`Value` reconstruction.
    assert!(
        Rc::ptr_eq(&first, &second),
        "tagged cache hit should share the cached record"
    );
    let TaggedValue::Obj(map) = &*first else {
        panic!("module record should be a tagged object, got {first:?}");
    };
    assert!(map.contains_key("inc"), "record should export `inc`");
    assert!(
        map.contains_key("forty_two"),
        "record should export `forty_two`"
    );

    // The `Value` boundary path still produces the export record (and the
    // arrow serializes to its `__arrow__` form for cross-engine use).
    let value = loader.resolve_import("m.hnl").expect("value load");
    let Value::Object(obj) = &value else {
        panic!("resolve_import should return an object, got {value}");
    };
    assert_eq!(obj.get("forty_two"), Some(&json!(42)));
    assert!(
        obj.get("inc").and_then(|v| v.get("__arrow__")).is_some(),
        "exported arrow should serialize as __arrow__: {value}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn imported_arrow_applies_on_first_and_cached_import() {
    let dir = scratch_dir("apply");
    std::fs::write(dir.join("math.hnl"), "export double := (x) ~> x * 2\n").expect("write module");

    let (_loader, rt) = loader_and_runtime(dir.clone());

    // First import: miss → eval + cache; second: hit → shared record.
    // Both must apply the exported arrow identically — the tagged cache
    // path is what the `import` op binds into scope.
    for round in 0..2 {
        let value = rt
            .run(&Source::new(
                r#"do { from "math.hnl" import double; double(21) }"#,
            ))
            .unwrap_or_else(|e| panic!("round {round}: import+apply failed: {e}"));
        assert_eq!(value, json!(42), "round {round}");
    }

    std::fs::remove_dir_all(&dir).ok();
}
