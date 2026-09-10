//! ADR-0103 continuation (Step 4): arrow-model delegation from the Rust
//! tree-walk [`ValueModel`] to a Handl-authored `.hnl` model.
//!
//! These tests construct a [`ValueModel`] via
//! [`ValueModel::with_arrow_model_delegate`] — wiring a
//! [`HandlModelAdapter`] backed by `arrow_model.hnl` as the delegate — and
//! verify that HoTT-type terms (`Path`, `Equiv`, `Refine`, `Rewrite`) flow
//! through the tree-walk engine's `Model::run` and produce results instead
//! of `NotImplemented`. The delegated results are compared against the
//! direct [`HandlModelAdapter`] output to confirm consistency, and against
//! the default (delegate-less) [`ValueModel`] to confirm the opt-in
//! delegation does not regress the bootstrap path.

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use handl_plugin_lib_handl::{HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{AstNode, Model, ModelCtx};
use serde_json::json;

const ARROW_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/arrow_model.hnl");

/// Build a `HandlModule` for the arrow model (mirrors `composite_model_parity`).
fn arrow_module() -> HandlModule {
    HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
}

/// Build a `HandlModelAdapter` from the arrow model module.
fn handl_adapter() -> HandlModelAdapter {
    HandlModelAdapter::new(arrow_module())
}

/// Build a `ValueModel` whose arrow-model delegate is the Handl-authored
/// `arrow_model.hnl` (the ADR-0103 Step 4 seam).
fn delegated_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(TreeWalkEngine::new().with_modules(modules))
        .with_arrow_model_delegate(handl_adapter())
}

/// Build the default (delegate-less) Rust `ValueModel` (bootstrap engine).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(TreeWalkEngine::new().with_modules(modules))
}

/// Run `term` through a `Model` impl with the given incoming value.
fn run_model<M: Model<Value = Value>>(model: &M, term: &Term, incoming: Value) -> Value {
    let ast = Ast::new();
    let lang = Language::new();
    let current_node = AstNode;
    let mut ctx = ModelCtx {
        ast: &ast,
        lang: &lang,
        current_node: &current_node,
        node_ref: 0,
        handler_stack: vec![],
    };
    model.run(term, incoming, &mut ctx).unwrap_or(Value::Null)
}

// -------------------------------------------------------------------------
// Path
// -------------------------------------------------------------------------

/// A delegated `Path` term produces a tagged `Path` record (not
/// `NotImplemented`), matching the direct `HandlModelAdapter` output.
#[test]
fn delegated_path_matches_adapter() {
    let term = Term::Path {
        endpoints: (
            Box::new(Term::Arr { value: json!(1) }),
            Box::new(Term::Arr { value: json!(2) }),
        ),
        witness: Some(Box::new(Term::Arr { value: json!(3) })),
    };
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let adapter = run_model(&handl_adapter(), &term, Value::Null);
    assert_eq!(
        delegated,
        json!({ "__tag__": "Path", "from": 1, "to": 2, "witness": 3 })
    );
    assert_eq!(delegated, adapter);
}

/// The delegated `Path` result also matches the default (delegate-less)
/// Rust `ValueModel`, confirming the delegate is a faithful replacement.
#[test]
fn delegated_path_matches_rust_default() {
    let term = Term::Path {
        endpoints: (
            Box::new(Term::Arr { value: json!(1) }),
            Box::new(Term::Arr { value: json!(2) }),
        ),
        witness: Some(Box::new(Term::Arr { value: json!(3) })),
    };
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let rust = run_model(&rust_model(), &term, Value::Null);
    assert_eq!(delegated, rust);
}

// -------------------------------------------------------------------------
// Equiv
// -------------------------------------------------------------------------

/// A delegated `Equiv` over equal endpoints evaluates to `true`, matching
/// the direct adapter.
#[test]
fn delegated_equiv_matches_adapter() {
    let term = Term::Equiv {
        a: Box::new(Term::Arr { value: json!(1) }),
        b: Box::new(Term::Arr { value: json!(1) }),
        path: None,
    };
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let adapter = run_model(&handl_adapter(), &term, Value::Null);
    assert_eq!(delegated, json!(true));
    assert_eq!(delegated, adapter);
}

/// The delegated `Equiv` result matches the default Rust `ValueModel`.
#[test]
fn delegated_equiv_matches_rust_default() {
    let term = Term::Equiv {
        a: Box::new(Term::Arr { value: json!(1) }),
        b: Box::new(Term::Arr { value: json!(1) }),
        path: None,
    };
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let rust = run_model(&rust_model(), &term, Value::Null);
    assert_eq!(delegated, rust);
}

// -------------------------------------------------------------------------
// Refine
// -------------------------------------------------------------------------

/// A delegated `Refine` whose predicate holds returns the incoming value,
/// matching the direct adapter.
#[test]
fn delegated_refine_matches_adapter() {
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Refine {
            sort: Box::new(Term::op_native("id", vec![])),
            predicate: Box::new(Term::op_native("is_pos", vec![])),
        },
    );
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let adapter = run_model(&handl_adapter(), &term, Value::Null);
    assert_eq!(delegated, json!(5));
    assert_eq!(delegated, adapter);
}

/// The delegated `Refine` result matches the default Rust `ValueModel`.
#[test]
fn delegated_refine_matches_rust_default() {
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Refine {
            sort: Box::new(Term::op_native("id", vec![])),
            predicate: Box::new(Term::op_native("is_pos", vec![])),
        },
    );
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let rust = run_model(&rust_model(), &term, Value::Null);
    assert_eq!(delegated, rust);
}

// -------------------------------------------------------------------------
// Rewrite
// -------------------------------------------------------------------------

/// A delegated `Rewrite` evaluates to the incoming value (the `Seq`-fed
/// payload), matching the direct adapter. Without the delegate, the Rust
/// `ValueModel` `Model::run` arm for `Rewrite` runs the target and returns
/// its result — this confirms the delegate path is exercised and agrees.
#[test]
fn delegated_rewrite_matches_adapter() {
    use handl_language::RewriteWitness;
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Rewrite {
            source: Box::new(Term::Arr { value: json!(1) }),
            target: Box::new(Term::op_native("id", vec![])),
            witness: RewriteWitness::Named("x".to_string()),
        },
    );
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let adapter = run_model(&handl_adapter(), &term, Value::Null);
    assert_eq!(delegated, json!(5));
    assert_eq!(delegated, adapter);
}

/// The delegated `Rewrite` result matches the default Rust `ValueModel`.
#[test]
fn delegated_rewrite_matches_rust_default() {
    use handl_language::RewriteWitness;
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Rewrite {
            source: Box::new(Term::Arr { value: json!(1) }),
            target: Box::new(Term::op_native("id", vec![])),
            witness: RewriteWitness::Named("x".to_string()),
        },
    );
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    let rust = run_model(&rust_model(), &term, Value::Null);
    assert_eq!(delegated, rust);
}

// -------------------------------------------------------------------------
// Opt-in: core tags still use the Rust engine
// -------------------------------------------------------------------------

/// A core arrow-calculus tag (`Seq` + `inc`) is *not* a HoTT tag, so the
/// delegate is bypassed and the Rust tree-walk engine handles it — even
/// when a delegate is configured. This guards the `is_hott_tag` boundary.
#[test]
fn delegated_model_still_handles_core_tags_in_rust() {
    let term = Term::seq(
        Term::Arr { value: json!(1) },
        Term::op_native("inc", vec![]),
    );
    let delegated = run_model(&delegated_model(), &term, Value::Null);
    assert_eq!(delegated, json!(2));
    assert_eq!(delegated, run_model(&rust_model(), &term, Value::Null));
}
