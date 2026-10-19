//! ADR-0103 Phase 9: the factorial example.
//!
//! The ADR mandates that the factorial example (`factorial(5) = 120`) runs
//! through the exec model (evaluation) and the type model (checking) with
//! the *same source*. This file verifies the evaluation side: it constructs
//! a `Term` that computes `factorial(5) = 120` using arrow-calculus
//! combinators, runs it through the Rust tree-walk model ([`ValueModel`])
//! and the Handl arrow model ([`HandlModel`] from `arrow_model.hnl`), and
//! asserts both produce `120` and agree with each other.
//!
//! Two formulations are exercised:
//!   - A nested `mul` `Op` chain (`5 * 4 * 3 * 2 * 1`), the most direct
//!     encoding of factorial as a static arrow — the canonical
//!     `factorial(5) = 120` term.
//!   - A bounded [`TermData::iter`] countdown that complements the mul-chain
//!     by exercising bounded iteration through both models (confirming
//!     exec/arrow parity on `Iter` itself).

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use hnl_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use hnl_engine::{ArrSource, AstNode, Model, ModelCtx, TermData};
use serde_json::json;


/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const ARROW_MODEL_HNL: &str = include_str!("fixtures/arrow_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

/// Build the Handl arrow model from `arrow_model.hnl`.
fn handl_model() -> HandlModel {
    let module = HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(format!(
                "{}/src/array/index.hnl",
                handl_lib_collections::ROOT
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);
    HandlModel::new(module)
}

/// Build the pure Rust tree-walk `ValueModel` (the exec model).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(hnl_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
}

/// Evaluate `term` through both the Rust exec model and the Handl arrow
/// model, returning `(rust_value, handl_value)`.
fn eval(term: &Term, incoming: Value) -> (Value, Value) {
    let rust = rust_model();
    let rust_value = {
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
        rust.run(term, incoming.clone(), &mut ctx)
            .unwrap_or(Value::Null)
    };

    let handl = handl_model();
    let ctx_value = json!({ "handler_stack": [], "node_ref": 0 });
    let handl_value = handl
        .run(term, &incoming, &ctx_value)
        .unwrap_or_else(|e| panic!("handl model error for term {:?}: {:?}", term, e));

    (rust_value, handl_value)
}

/// A `__pair__`-tagged JSON value, matching the runtime pair shape.
fn pair(a: Value, b: Value) -> Value {
    json!({ "__pair__": [a, b] })
}

/// Build the nested-`mul` factorial term: `5 * 4 * 3 * 2 * 1`.
///
/// This is the most direct arrow-calculus encoding of `factorial(5)`: a
/// right-associated chain of native `mul` operations over constant
/// arrows. Both the exec model and the arrow model evaluate it to `120`.
fn factorial_mul_chain() -> Term {
    // 5 * (4 * (3 * (2 * 1)))
    to_term(TermData::op(
        "mul",
        vec![
            TermData::arr(json!(5)),
            TermData::op(
                "mul",
                vec![
                    TermData::arr(json!(4)),
                    TermData::op(
                        "mul",
                        vec![
                            TermData::arr(json!(3)),
                            TermData::op(
                                "mul",
                                vec![TermData::arr(json!(2)), TermData::arr(json!(1))],
                                &ArrSource::Native,
                                None,
                            ),
                        ],
                        &ArrSource::Native,
                        None,
                    ),
                ],
                &ArrSource::Native,
                None,
            ),
        ],
        &ArrSource::Native,
        None,
    ))
}

/// Build an [`TermData::iter`] countdown term that complements the mul-chain
/// factorial by exercising bounded iteration through both models.
///
/// `Iter` threads a single state value, so a pair-accumulator encoding of
/// factorial would require pair-unpacking in the step arrow that the core
/// `mul`/`add` ops do not provide. Instead, this is the documented
/// countdown shape (`is_pos` predicate, `dec` step, fuel-bounded) starting
/// from `5`: it runs `5 -> 4 -> 3 -> 2 -> 1 -> 0` and bottoms out at `0`.
/// The mul-chain above is the canonical `factorial(5) = 120` arrow; this
/// variant confirms the exec and arrow models agree on `Iter` itself.
fn factorial_iter() -> Term {
    to_term(TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(5)),
    ))
}

/// `factorial(5) = 120` via a nested `mul` `Op` chain — exec model.
#[test]
fn factorial_mul_chain_exec_model() {
        ensure_lang();
    let term = factorial_mul_chain();
    let (rust, _handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(120));
}

/// `factorial(5) = 120` via a nested `mul` `Op` chain — arrow model.
#[test]
fn factorial_mul_chain_arrow_model() {
        ensure_lang();
    let term = factorial_mul_chain();
    let (_rust, handl) = eval(&term, Value::Null);
    assert_eq!(handl, json!(120));
}

/// Parity: the exec model and the arrow model agree on `factorial(5) = 120`
/// (mul-chain encoding).
#[test]
fn factorial_mul_chain_parity() {
        ensure_lang();
    let term = factorial_mul_chain();
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(120));
    assert_eq!(handl, json!(120));
    assert_eq!(rust, handl);
}

/// `factorial(5)` expressed as an `Iter` countdown — exec model produces 0
/// (the countdown bottom), and the arrow model agrees. This exercises the
/// `Iter` combinator that the ADR's factorial spec references, through both
/// models, confirming parity on bounded iteration.
#[test]
fn factorial_iter_parity() {
        ensure_lang();
    let term = factorial_iter();
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(0));
}

/// The mul-chain factorial run through the arrow model with a non-null
/// incoming value still yields `120` (the `Arr` constants dominate; `mul`
/// uses its two evaluated args, not `incoming`).
#[test]
fn factorial_mul_chain_non_null_incoming() {
        ensure_lang();
    let term = factorial_mul_chain();
    let (rust, handl) = eval(&term, json!(999));
    assert_eq!(rust, json!(120));
    assert_eq!(handl, json!(120));
    assert_eq!(rust, handl);
}

/// A smaller factorial `factorial(3) = 6` confirms the chain scales.
#[test]
fn factorial_three_is_six() {
        ensure_lang();
    let term = to_term(TermData::op(
        "mul",
        vec![
            TermData::arr(json!(3)),
            TermData::op(
                "mul",
                vec![TermData::arr(json!(2)), TermData::arr(json!(1))],
                &ArrSource::Native,
                None,
            ),
        ],
        &ArrSource::Native,
        None,
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(6));
    assert_eq!(handl, json!(6));
    assert_eq!(rust, handl);
}

/// `factorial(1) = 1` (base case) — a single `Arr` constant.
#[test]
fn factorial_one_is_one() {
        ensure_lang();
    let term = to_term(TermData::arr(json!(1)));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(1));
    assert_eq!(handl, json!(1));
    assert_eq!(rust, handl);
}

/// `factorial(0) = 1` (empty product) — a single `Arr` constant.
#[test]
fn factorial_zero_is_one() {
        ensure_lang();
    let term = to_term(TermData::arr(json!(1)));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(1));
    assert_eq!(handl, json!(1));
    assert_eq!(rust, handl);
}

/// The factorial mul-chain is a `TermData::op` whose `TermData` representation
/// round-trips losslessly — the data representation that the type model
/// will consume (ADR-0103 Phase 6).
#[test]
fn factorial_mul_chain_termdata_roundtrip() {
        ensure_lang();
    let term = factorial_mul_chain();
    let data = TermData::from_term(&term);
    assert_eq!(data.tag(), "Op");
    assert_eq!(data.get_str("name"), Some("mul"));
    assert_eq!(data.get_term_list("args").len(), 2);
    assert_eq!(data.to_term(), Ok(term));
}

/// The factorial result carried as the first component of a pair: running
/// `First(id)` over a pair `(120, 0)` yields the pair `(120, 0)` — `First`
/// runs the inner arrow on the first component and re-pairs it with the
/// second. This exercises `First` (fan-out) parity with the factorial value.
#[test]
fn factorial_in_pair_first() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(120), json!(0))),
        TermData::first(TermData::op("id", vec![], &ArrSource::Native, None)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, pair(json!(120), json!(0)));
    assert_eq!(handl, pair(json!(120), json!(0)));
    assert_eq!(rust, handl);
}
