//! ADR-0103 Phase 9: model infrastructure integration tests.
//!
//! These tests verify the model infrastructure that ties the Rust
//! bootstrap engines to the Handl-authored data models:
//!
//!   - [`HandlModelAdapter`] can be built from `arrow_model.hnl` and run as
//!     a [`Model`].
//!   - [`CompositeModel`] with [`is_hott_tag`] delegates HoTT-specific tags
//!     to the Handl adapter while keeping core arrow-calculus tags in the
//!     Rust [`ValueModel`].
//!   - [`TermData`] round-trips for key `Term` variants (the data
//!     representation the type model consumes).
//!   - The arrow model, value model, and composite model all agree on
//!     basic cases (exec-vs-arrow and exec-vs-type parity).

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{
    ArrSource, AstNode, CompositeDataModel, CompositeModel, Model, ModelCtx, ModelData, TermData,
    is_hott_tag,
};
use serde_json::json;

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.

/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

const ARROW_MODEL_HNL: &str = include_str!("fixtures/arrow_model.hnl");

/// Build a `HandlModule` for the arrow model.
fn arrow_module() -> HandlModule {
    HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../handl-packages/collections/array/index.hnl"
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
}

/// Build a `HandlModelAdapter` from the arrow model module.
fn handl_adapter() -> HandlModelAdapter {
    HandlModelAdapter::new(arrow_module())
}

/// Build a `HandlModel` from the arrow model module.
fn handl_model() -> HandlModel {
    HandlModel::new(arrow_module())
}

/// Build the pure Rust `ValueModel` (bootstrap engine).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
}

/// Build the composite model: `ValueModel` (primary) + `HandlModelAdapter`
/// (delegate), using `is_hott_tag` as the delegation predicate.
fn composite_model() -> CompositeModel<ValueModel, HandlModelAdapter> {
    CompositeModel::new(rust_model(), handl_adapter(), is_hott_tag)
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

/// Run `term` through the `HandlModel` (the raw arrow model).
fn run_handl(term: &Term, incoming: Value) -> Value {
    let model = handl_model();
    let ctx_value = json!({ "handler_stack": [], "node_ref": 0 });
    model
        .run(term, &incoming, &ctx_value)
        .unwrap_or_else(|e| panic!("handl model error for term {:?}: {:?}", term, e))
}

// -------------------------------------------------------------------------
// HandlModelAdapter
// -------------------------------------------------------------------------

/// `HandlModelAdapter` can be created from `arrow_model.hnl` and runs a
/// constant arrow.
#[test]
fn handl_adapter_runs_arr_const() {
        ensure_lang();
    let adapter = handl_adapter();
    let term = to_term(TermData::arr(json!(5)));
    let result = run_model(&adapter, &term, Value::Null);
    assert_eq!(result, json!(5));
}

/// `HandlModelAdapter` runs a sequential composition.
#[test]
fn handl_adapter_runs_seq() {
        ensure_lang();
    let adapter = handl_adapter();
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let result = run_model(&adapter, &term, Value::Null);
    assert_eq!(result, json!(2));
}

/// `HandlModelAdapter` runs an arithmetic `Op`.
#[test]
fn handl_adapter_runs_op_add() {
        ensure_lang();
    let adapter = handl_adapter();
    let term = to_term(TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    ));
    let result = run_model(&adapter, &term, Value::Null);
    assert_eq!(result, json!(3));
}

/// `HandlModelAdapter` agrees with the raw `HandlModel` on a constant.
#[test]
fn handl_adapter_matches_handl_model() {
        ensure_lang();
    let adapter = handl_adapter();
    let term = to_term(TermData::arr(json!(42)));
    let adapter_result = run_model(&adapter, &term, Value::Null);
    let handl_result = run_handl(&term, Value::Null);
    assert_eq!(adapter_result, handl_result);
    assert_eq!(adapter_result, json!(42));
}

// -------------------------------------------------------------------------
// CompositeModel + is_hott_tag delegation
// -------------------------------------------------------------------------

/// `is_hott_tag` returns `true` for HoTT-specific tags.
#[test]
fn is_hott_tag_true_for_hott_tags() {
        ensure_lang();
    assert!(TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None,).is_hott_tag());
    assert!(TermData::equiv(TermData::arr(json!(1)), TermData::arr(json!(1)), None,).is_hott_tag());
    assert!(TermData::cell_id(TermData::arr(json!(1))).is_hott_tag());
    assert!(
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        )
        .is_hott_tag()
    );
}

/// `is_hott_tag` returns `false` for core arrow-calculus tags.
#[test]
fn is_hott_tag_false_for_core_tags() {
        ensure_lang();
    assert!(!TermData::arr(json!(5)).is_hott_tag());
    assert!(
        !TermData::seq(
            TermData::arr(json!(1)),
            TermData::op("inc", vec![], &ArrSource::Native, None),
        )
        .is_hott_tag()
    );
    assert!(!TermData::op("add", vec![], &ArrSource::Native, None).is_hott_tag());
    assert!(!TermData::tag_value("ok", TermData::arr(json!(1))).is_hott_tag());
}

/// The composite model delegates HoTT tags to the Handl adapter and
/// matches the pure Rust model on a `Path` term.
#[test]
fn composite_delegates_hott_path() {
        ensure_lang();
    let composite = composite_model();
    let rust = rust_model();
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
    let composite_result = run_model(&composite, &term, Value::Null);
    let rust_result = run_model(&rust, &term, Value::Null);
    assert_eq!(composite_result, rust_result);
    assert_eq!(
        composite_result,
        json!({ "__tag__": "Path", "from": 1, "to": 2, "witness": 3 })
    );
}

/// The composite model uses the Rust model for core tags (Seq + inc).
#[test]
fn composite_uses_rust_for_core_tags() {
        ensure_lang();
    let composite = composite_model();
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let result = run_model(&composite, &term, Value::Null);
    assert_eq!(result, json!(2));
}

/// The composite model agrees with the Rust model on a `Refine` (HoTT)
/// tag — confirming the delegated path produces the same value.
#[test]
fn composite_refine_matches_rust() {
        ensure_lang();
    let composite = composite_model();
    let rust = rust_model();
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        ),
    ));
    let composite_result = run_model(&composite, &term, Value::Null);
    let rust_result = run_model(&rust, &term, Value::Null);
    assert_eq!(composite_result, rust_result);
    assert_eq!(composite_result, json!(5));
}

// -------------------------------------------------------------------------
// TermData round-trip for key variants
// -------------------------------------------------------------------------

/// `TermData` round-trips for `Arr`.
#[test]
fn termdata_roundtrip_arr() {
        ensure_lang();
    let data = TermData::arr(json!(5));
    assert_eq!(data.tag(), "Arr");
    assert_eq!(data.get("value"), Some(&json!(5)));
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

/// `TermData` round-trips for `Seq`.
#[test]
fn termdata_roundtrip_seq() {
        ensure_lang();
    let data = TermData::seq(TermData::arr(json!(1)), TermData::arr(json!(2)));
    assert_eq!(data.tag(), "Seq");
    assert_eq!(data.get_term("fst").unwrap().tag(), "Arr");
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

/// `TermData` round-trips for `Op` (native, with args).
#[test]
fn termdata_roundtrip_op() {
        ensure_lang();
    let data = TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    );
    assert_eq!(data.tag(), "Op");
    assert_eq!(data.get_str("name"), Some("add"));
    assert_eq!(data.get_term_list("args").len(), 2);
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

/// `TermData` round-trips for `Iter`.
#[test]
fn termdata_roundtrip_iter() {
        ensure_lang();
    let data = TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(5)),
    );
    assert_eq!(data.tag(), "Iter");
    assert_eq!(data.get_term("fuel").unwrap().tag(), "Arr");
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

/// `TermData` round-trips for `Path` (HoTT tag).
#[test]
fn termdata_roundtrip_path() {
        ensure_lang();
    let data = TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None);
    assert_eq!(data.tag(), "Path");
    assert_eq!(data.get_term("from").unwrap().get("value"), Some(&json!(1)));
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

/// `TermData` round-trips for `Tag`.
#[test]
fn termdata_roundtrip_tag() {
        ensure_lang();
    let data = TermData::tag_value("ok", TermData::arr(json!(42)));
    assert_eq!(data.tag(), "Tag");
    assert_eq!(data.get_str("tag"), Some("ok"));
    let round = data.to_term().unwrap();
    assert_eq!(TermData::from_term(&round), data);
}

// -------------------------------------------------------------------------
// Cross-model agreement: arrow vs value vs composite
// -------------------------------------------------------------------------

/// The arrow model, value model, and composite model all agree on a
/// constant arrow.
#[test]
fn all_models_agree_arr_const() {
        ensure_lang();
    let term = to_term(TermData::arr(json!(5)));
    let rust = run_model(&rust_model(), &term, Value::Null);
    let handl = run_handl(&term, Value::Null);
    let composite = run_model(&composite_model(), &term, Value::Null);
    assert_eq!(rust, json!(5));
    assert_eq!(handl, json!(5));
    assert_eq!(composite, json!(5));
    assert_eq!(rust, handl);
    assert_eq!(rust, composite);
}

/// All three models agree on `Seq` + `inc`.
#[test]
fn all_models_agree_seq_inc() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let rust = run_model(&rust_model(), &term, Value::Null);
    let handl = run_handl(&term, Value::Null);
    let composite = run_model(&composite_model(), &term, Value::Null);
    assert_eq!(rust, json!(2));
    assert_eq!(handl, json!(2));
    assert_eq!(composite, json!(2));
    assert_eq!(rust, handl);
    assert_eq!(rust, composite);
}

/// All three models agree on `add` over two constants.
#[test]
fn all_models_agree_op_add() {
        ensure_lang();
    let term = to_term(TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    ));
    let rust = run_model(&rust_model(), &term, Value::Null);
    let handl = run_handl(&term, Value::Null);
    let composite = run_model(&composite_model(), &term, Value::Null);
    assert_eq!(rust, json!(3));
    assert_eq!(handl, json!(3));
    assert_eq!(composite, json!(3));
    assert_eq!(rust, handl);
    assert_eq!(rust, composite);
}

/// All three models agree on `Iter` (countdown).
#[test]
fn all_models_agree_iter() {
        ensure_lang();
    let term = to_term(TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(3)),
    ));
    let rust = run_model(&rust_model(), &term, Value::Null);
    let handl = run_handl(&term, Value::Null);
    let composite = run_model(&composite_model(), &term, Value::Null);
    assert_eq!(rust, json!(0));
    assert_eq!(handl, json!(0));
    assert_eq!(composite, json!(0));
    assert_eq!(rust, handl);
    assert_eq!(rust, composite);
}

/// All three models agree on a HoTT `Path` term (composite delegates to
/// the Handl adapter, which matches the Rust model).
#[test]
fn all_models_agree_path() {
        ensure_lang();
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
    let rust = run_model(&rust_model(), &term, Value::Null);
    let handl = run_handl(&term, Value::Null);
    let composite = run_model(&composite_model(), &term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, composite);
    assert_eq!(
        rust,
        json!({ "__tag__": "Path", "from": 1, "to": 2, "witness": 3 })
    );
}

// -------------------------------------------------------------------------
// ADR-0103 continuation: TermData::is_hott_tag + CompositeDataModel
// -------------------------------------------------------------------------

/// `TermData::is_hott_tag()` returns `true` for HoTT-specific tags.
#[test]
fn termdata_is_hott_tag_true_for_hott_tags() {
        ensure_lang();
    assert!(TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None).is_hott_tag());
    assert!(TermData::equiv(TermData::arr(json!(1)), TermData::arr(json!(1)), None).is_hott_tag());
    assert!(TermData::cell_id(TermData::arr(json!(1))).is_hott_tag());
    assert!(TermData::src(TermData::arr(json!(1))).is_hott_tag());
    assert!(TermData::tgt(TermData::arr(json!(1))).is_hott_tag());
    assert!(TermData::cell_comp_v(TermData::arr(json!(1)), TermData::arr(json!(2))).is_hott_tag());
    assert!(
        TermData::whisker(
            &plugin_engine::Side::Left,
            TermData::arr(json!(1)),
            TermData::arr(json!(2)),
        )
        .is_hott_tag()
    );
    assert!(TermData::univalence(TermData::arr(json!(1))).is_hott_tag());
    assert!(TermData::glue(TermData::arr(json!(1)), TermData::arr(json!(2))).is_hott_tag());
    assert!(TermData::fibrant(TermData::arr(json!(1)), TermData::arr(json!(2))).is_hott_tag());
    assert!(
        TermData::jelim(
            TermData::arr(json!(1)),
            TermData::arr(json!(2)),
            TermData::arr(json!(3)),
        )
        .is_hott_tag()
    );
    assert!(
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        )
        .is_hott_tag()
    );
    assert!(TermData::cobind(TermData::arr(json!(1)), TermData::arr(json!(2))).is_hott_tag());
    assert!(
        TermData::rewrite(
            TermData::arr(json!(1)),
            TermData::arr(json!(2)),
            &plugin_engine::RewriteWitness::Compute,
        )
        .is_hott_tag()
    );
}

/// `TermData::is_hott_tag()` returns `false` for core arrow-calculus tags.
#[test]
fn termdata_is_hott_tag_false_for_core_tags() {
        ensure_lang();
    assert!(!TermData::arr(json!(5)).is_hott_tag());
    assert!(
        !TermData::seq(
            TermData::arr(json!(1)),
            TermData::op("inc", vec![], &ArrSource::Native, None),
        )
        .is_hott_tag()
    );
    assert!(!TermData::op("add", vec![], &ArrSource::Native, None).is_hott_tag());
    assert!(!TermData::tag_value("ok", TermData::arr(json!(1))).is_hott_tag());
    assert!(!TermData::first(TermData::arr(json!(1))).is_hott_tag());
}

/// `TermData::tag()` extracts the `"__tag__"` field as a string and returns
/// the empty string when the field is absent.
#[test]
fn termdata_tag_extracts_tag_string() {
        ensure_lang();
    assert_eq!(TermData::arr(json!(5)).tag(), "Arr");
    assert_eq!(
        TermData::op("add", vec![], &ArrSource::Native, None).tag(),
        "Op"
    );
    assert_eq!(
        TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None).tag(),
        "Path"
    );
    // A bare value (not a tagged record) yields the empty string.
    assert_eq!(TermData(json!("bare")).tag(), "");
}

/// A mock primary [`ModelData`] that tags its result with `"primary"`.
#[derive(Clone)]
struct PrimaryData;
impl ModelData for PrimaryData {
    fn run_data(
        &self,
        term: &TermData,
        _incoming: &Value,
        _ctx: &ModelCtx,
    ) -> Result<Value, plugin_engine::LangError> {
        Ok(json!({ "model": "primary", "tag": term.tag() }))
    }
}

/// A mock delegate [`ModelData`] that tags its result with `"delegate"`.
#[derive(Clone)]
struct DelegateData;
impl ModelData for DelegateData {
    fn run_data(
        &self,
        term: &TermData,
        _incoming: &Value,
        _ctx: &ModelCtx,
    ) -> Result<Value, plugin_engine::LangError> {
        Ok(json!({ "model": "delegate", "tag": term.tag() }))
    }
}

/// Owned bundle of context dependencies for data-driven model tests.
struct CtxBundle {
    ast: handl_language::Ast,
    lang: handl_language::Language,
    node: AstNode,
}

impl CtxBundle {
    /// Build a `ModelCtx` borrowing this bundle's owned values.
    fn ctx(&self) -> ModelCtx<'_> {
        ModelCtx {
            ast: &self.ast,
            lang: &self.lang,
            current_node: &self.node,
            node_ref: 0,
            handler_stack: vec![],
        }
    }
}

/// Build a `CtxBundle` for data-driven model tests.
fn data_ctx() -> CtxBundle {
    CtxBundle {
        ast: handl_language::Ast::new(),
        lang: handl_language::Language::new(),
        node: AstNode,
    }
}

/// `CompositeDataModel` routes HoTT tags to the delegate and core tags to
/// the primary when using `TermData::is_hott_tag` as the predicate.
#[test]
fn composite_data_model_delegates_hott_tags() {
        ensure_lang();
    let composite = CompositeDataModel::new(PrimaryData, DelegateData, TermData::is_hott_tag);

    // HoTT tag → delegate.
    let hott = TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None);
    let bundle = data_ctx();
    let ctx = bundle.ctx();
    let result = composite.run_data(&hott, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "delegate", "tag": "Path" }));

    // Core tag → primary.
    let core = TermData::arr(json!(5));
    let result = composite.run_data(&core, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "primary", "tag": "Arr" }));

    // Another core tag → primary.
    let core_seq = TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    );
    let result = composite.run_data(&core_seq, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "primary", "tag": "Seq" }));

    // Another HoTT tag → delegate.
    let refine = TermData::refine(
        TermData::op("id", vec![], &ArrSource::Native, None),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
    );
    let result = composite.run_data(&refine, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "delegate", "tag": "Refine" }));
}

/// `CompositeDataModel` agrees with a custom predicate that delegates a
/// non-HoTT tag, confirming the predicate is pluggable.
#[test]
fn composite_data_model_custom_predicate() {
        ensure_lang();
    fn delegates_arr(term: &TermData) -> bool {
        term.is_tag("Arr")
    }
    let composite = CompositeDataModel::new(PrimaryData, DelegateData, delegates_arr);

    let arr = TermData::arr(json!(5));
    let bundle = data_ctx();
    let ctx = bundle.ctx();
    let result = composite.run_data(&arr, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "delegate", "tag": "Arr" }));

    let path = TermData::path(TermData::arr(json!(1)), TermData::arr(json!(2)), None);
    let result = composite.run_data(&path, &Value::Null, &ctx).unwrap();
    assert_eq!(result, json!({ "model": "primary", "tag": "Path" }));
}
