//! ADR-0175 run layer: the `pipeline` module of meta-tier stage-ops
//! (`lift`/`search`/`verify`/`deliver`) registered by `handl-kit-mlopt`,
//! evaluated as a `lift(...) >>> search >>> verify >>> deliver`
//! surface pipeline.
//!
//! This file graduated from spike to integration coverage when
//! `crates/kit-mlopt` landed: the stage-ops are real `Function` impls
//! that spawn `$MLOPT_PIPELINE_CMD <stage>` (default
//! `python3 -m mlopt stage <stage>`) over a JSON stdin/stdout protocol.
//! The tests point `MLOPT_PIPELINE_CMD` at
//! `crates/kit-mlopt/tests/fixtures/stage_stub.py`, a Python endpoint
//! that threads the record exactly like the pending mlopt-side
//! `stage` subcommand will.
//!
//! What is pinned here:
//!
//! - **Surface `>>>` needs no `id`-arg workaround.** `a >>> f` is App
//!   scheme (`f(a)`), so a stage-op is an ordinary `Value -> Value`
//!   function; the pipeline record is just `args[0]`.
//! - **`hnl run` sees the module.** `pipeline` is in the prelude
//!   manifest's `base_modules` (kit-data-prelude `prelude.yaml`), so
//!   `Kit::run` — what `hnl run` evaluates through — resolves the flat
//!   stage wires and the subprocess Function runs end to end.
//!
//! Report: `docs/reports/2026-11-15-hnl-run-meta-tier-kit.md`.

use std::rc::Rc;
use std::sync::Once;

use handl_kit_prelude::Kit;
use handl_platform::ModuleLookup;
use hnl_engine_exec::{EvalEngine, EvalRequest};
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use serde_json::json;

/// Point the stage-ops at the fixture endpoint for every eval in this
/// test binary. `Once` + a single write: edition-2024 `set_var` is
/// unsafe, and all tests here share the same stub so concurrent reads
/// are consistent.
fn stub_env() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: single write via `Once`; every consumer in this
        // binary reads the same value.
        unsafe {
            std::env::set_var(
                "MLOPT_PIPELINE_CMD",
                format!(
                    "python3 {}/../kit-mlopt/tests/fixtures/stage_stub.py",
                    env!("CARGO_MANIFEST_DIR")
                ),
            );
        }
    });
}

/// The eval-time registry over every registered module — the
/// `build_registry(None)` path, with `handl-kit-mlopt`'s `pipeline`
/// module contributing `lift`/`search`/`verify`.
fn pipeline_registry() -> handl_plugin_module::ModuleRegistry {
    handl_kit_mlopt::register();
    handl_kit_core::build_registry(None, "tree-walk").registry
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn pipeline_module_registers_and_resolves_qualified_and_flat() {
    stub_env();
    let registry = pipeline_registry();
    // Qualified wires: `pipeline.lift` etc.
    assert!(registry.lookup("pipeline.lift").is_some());
    assert!(registry.lookup("pipeline.search").is_some());
    assert!(registry.lookup("pipeline.verify").is_some());
    // Flat wires: `lift`/`search`/`verify` via the all-module search —
    // this is what lets a script write bare stage names.
    assert!(registry.lookup("lift").is_some());
    assert!(registry.lookup("search").is_some());
    assert!(registry.lookup("verify").is_some());
    assert!(registry.lookup("deliver").is_some());
    assert!(registry.lookup("pipeline.deliver").is_some());
    // Unknown stage fails loudly at lookup, not at eval.
    assert!(registry.lookup("pipeline.bench").is_none());
}

#[test]
fn surface_pipeline_evals_through_registered_module() {
    stub_env();
    let kit = Kit::new().expect("canonical kit must assemble");
    // The ADR-0175 script shape: `lift(model) >>> search >>> verify`.
    // Surface `>>>` is App scheme — parses to `verify(search(lift(...)))`.
    let ast = kit
        .parse(r#"lift("demo.torch") >>> search >>> verify"#)
        .expect("pipeline source must parse");

    let engine =
        TreeWalkEngine::new().with_modules(Rc::new(pipeline_registry()) as Rc<dyn ModuleLookup>);
    let result = engine
        .eval(kit.lang(), &EvalRequest::new(ast))
        .expect("pipeline must eval");

    // The stub threads the record: lift mints {model: "demo.torch"},
    // search adds laws_applied, verify gates on it and reports the model.
    assert_eq!(
        result,
        json!({
            "stage": "verify",
            "ok": true,
            "model": "demo.torch",
            "verified": {"passed": true},
        })
    );
}

#[test]
fn unregistered_stage_fails_loudly() {
    stub_env();
    let kit = Kit::new().expect("canonical kit must assemble");
    let ast = kit
        .parse(r#"lift("demo.torch") >>> nosuchstage"#)
        .expect("pipeline source must parse");
    let engine =
        TreeWalkEngine::new().with_modules(Rc::new(pipeline_registry()) as Rc<dyn ModuleLookup>);
    let err = engine
        .eval(kit.lang(), &EvalRequest::new(ast))
        .expect_err("unregistered stage must fail");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("nosuchstage") || msg.contains("not found"),
        "unexpected error: {msg}"
    );
}

/// The `hnl run` gap is closed: `pipeline` is in the prelude manifest's
/// `base_modules`, so `Kit::run` (what `hnl run` evaluates through)
/// resolves the stage wires and the subprocess Function runs end to
/// end against the fixture endpoint. Without `MLOPT_PIPELINE_CMD` the
/// same call would spawn `python3 -m mlopt stage …` and fail loudly
/// until mlopt grows the `stage` subcommand — see the crate docs in
/// `crates/kit-mlopt/src/lib.rs` for the endpoint spec.
#[test]
fn canonical_kit_run_evals_pipeline_stage_ops() {
    stub_env();
    let kit = Kit::new().expect("canonical kit must assemble");
    let result = kit
        .run(r#"lift("demo.torch") >>> search >>> verify"#)
        .expect("canonical kit must eval pipeline stage-ops");
    assert_eq!(
        result,
        json!({
            "stage": "verify",
            "ok": true,
            "model": "demo.torch",
            "verified": {"passed": true},
        })
    );
}

/// The full ADR-0175 pipeline: `deliver` wants the record's `out`
/// artifact path, which no stage mints — the script injects it between
/// `verify` and `deliver` with an object-spread record update, the same
/// shape a real pipeline script uses.
#[test]
fn canonical_kit_run_evals_full_pipeline_with_deliver() {
    stub_env();
    let kit = Kit::new().expect("canonical kit must assemble");
    let result = kit
        .run(
            r#"lift("demo.torch") >>> search >>> verify
               >>> (r ~> {...r, "out": "delivered.hnl"})
               >>> deliver"#,
        )
        .expect("canonical kit must eval the full pipeline");
    assert_eq!(result["stage"], "deliver");
    assert_eq!(result["delivered"], true);
    assert_eq!(result["artifact"]["path"], "delivered.hnl");
}
