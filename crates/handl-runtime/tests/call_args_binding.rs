//! Regression tests for `Runtime::eval_shared_with_call_args` (ADR-0106 env
//! removal): arguments are bound directly on the call frame's scope — without
//! being injected as per-call engine globals (`with_global` cloning) and
//! without leaking into the engine's persistent `globals` state between calls.
//!
//! The tree-walk engine overrides `EvalEngine::eval_shared_with_call_args` to
//! merge `__arg_N` bindings into the single `ScopeStack` globals map for the
//! call (excluded from `capture()`, so no env capture). This suite pins that
//! (a) args resolve, and (b) they do NOT persist on the engine across calls.

use std::rc::Rc;

use handl_language::Source;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};

/// Build a tree-walk runtime over the canonical data kit (no typecheck).
fn tree_walk_runtime() -> Runtime {
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

/// Parse `src` into an owned `Ast`, wrapped in an `Rc` for the shared-apply
/// seam.
fn parse_rc(rt: &Runtime, src: &str) -> Rc<handl_language::Ast> {
    let toks = rt.lex(&Source::new(src)).expect("lex");
    Rc::new(rt.parse(&toks).expect("parse"))
}

#[test]
fn call_args_bind_directly_without_globals() {
    let rt = tree_walk_runtime();
    let ast = parse_rc(&rt, "__arg_0 + 1");
    let args = vec![("__arg_0".to_string(), serde_json::json!(41))];
    let result = rt
        .eval_shared_with_call_args(&ast, &args)
        .expect("eval with call args");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn call_args_do_not_leak_into_engine_globals() {
    let rt = tree_walk_runtime();

    // First call binds `__arg_0 = 41` and succeeds.
    let ast = parse_rc(&rt, "__arg_0 + 1");
    let args = vec![("__arg_0".to_string(), serde_json::json!(41))];
    let first = rt
        .eval_shared_with_call_args(&ast, &args)
        .expect("first eval with call args");
    assert_eq!(first, serde_json::json!(42));

    // A second, globals-free eval of a bare `__arg_0` reference must NOT see the
    // previous call's arg. Unbound vars fall back to their own name as a
    // string; if args had leaked into the engine's persistent `globals` (the
    // old `with_global`-per-arg path), this would resolve to 41 instead.
    let bare = parse_rc(&rt, "__arg_0");
    let second = rt.eval(bare.as_ref()).expect("eval bare var");
    assert_eq!(second, serde_json::json!("__arg_0"));
    assert_ne!(second, serde_json::json!(41));
}

#[test]
fn call_args_are_per_call_not_per_engine() {
    let rt = tree_walk_runtime();
    let ast = parse_rc(&rt, "__arg_0 * 2");

    let a = rt
        .eval_shared_with_call_args(&ast, &[("__arg_0".to_string(), serde_json::json!(21))])
        .expect("eval a");
    let b = rt
        .eval_shared_with_call_args(&ast, &[("__arg_0".to_string(), serde_json::json!(3))])
        .expect("eval b");

    assert_eq!(a, serde_json::json!(42));
    assert_eq!(b, serde_json::json!(6));
}
