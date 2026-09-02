//! End-to-end `loop` tests across all three eval engines
//! (tree-walk, bytecode, cranelift).
//!
//! ADR-0052 specifies `loop` as the primitive for feedback — the kernel's
//! `Loop` combinator as surface syntax. The surface `loop` lowers to
//! `Seq(proj_child(0), Loop(loop_step(proj_child(1))))`, where `loop_step`
//! is a handler-interpreted derived step op. These tests verify that the
//! `loop_step` derived op evaluates identically under the tree-walk,
//! bytecode, and cranelift engines by running real `.hnl` source through
//! the canonical kit with each engine selected as the eval backend.

use handl_kit_data::{DataKit, DataKitManifest, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_language::Source;
use handl_plugin_engine_eval_bytecode::BytecodeEngine;
use handl_plugin_engine_eval_cranelift::CraneliftEngine;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, RuntimeConfig};
use serde_json::json;
use std::rc::Rc;

/// A canonical-kit manifest that selects `eval` as its eval engine.
fn manifest_with_eval(eval: &str) -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter(
            "canonical-yaml",
            "canonical-yaml",
        ))
        .with_runtime(
            RuntimeConfig::new()
                .with_lex("data")
                .with_parse("pratt")
                .with_eval(eval),
        )
}

/// Build a runtime over the full canonical language with the requested eval
/// engine bound. The module chain is `None` — the `loop` examples only use
/// core ops (`if`, `loop`, object/array literals, arithmetic) and no
/// foreign host functions.
fn runtime_with_eval(eval: &str) -> handl_runtime::Runtime {
    let reg = handl_kit_data_canonical::registry();
    let mut registry = EngineRegistry::new();
    registry.register_lex("data", Box::new(DataLexerEngine::new()));
    registry.register_parse("pratt", PrattEngine::new());
    registry.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    registry.register_eval("bytecode", Box::new(BytecodeEngine::new()));
    registry.register_eval("cranelift", Box::new(CraneliftEngine::new()));
    let registry = Rc::new(registry);
    let kit = DataKit::assemble(&manifest_with_eval(eval), &reg)
        .expect("canonical kit assembles");
    kit.into_runtime(&registry, None)
        .expect("runtime binds engines")
}

/// `(source, expected)` cases drawn from `examples/handl/*.hnl` — the
/// canonical `loop` corpus. Each runs under every engine below.
fn cases() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        // factorial.hnl — loop with destructured accumulator.
        (
            include_str!("../../../examples/handl/factorial.hnl"),
            json!(120),
        ),
        // fibonacci.hnl — loop with 3 destructured vars.
        (
            include_str!("../../../examples/handl/fibonacci.hnl"),
            json!(55),
        ),
        // sum-to-n.hnl — loop destructured accumulator.
        (
            include_str!("../../../examples/handl/sum-to-n.hnl"),
            json!(5050),
        ),
        // gcd.hnl — Euclidean GCD with destructured params.
        (
            include_str!("../../../examples/handl/gcd.hnl"),
            json!(12),
        ),
        // power.hnl — loop with 3 destructured vars, 2^10.
        (
            include_str!("../../../examples/handl/power.hnl"),
            json!(1024),
        ),
        // collatz.hnl — Collatz step count from 27.
        (
            include_str!("../../../examples/handl/collatz.hnl"),
            json!(111),
        ),
    ]
}

/// All three engines agree with the expected result for every `loop` case.
#[test]
fn loop_step_runs_under_tree_walk() {
    let rt = runtime_with_eval("tree-walk");
    for (src, expected) in cases() {
        let result = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("tree-walk eval failed for `{src}`: {e}"));
        assert_eq!(result, expected, "tree-walk mismatch for `{src}`");
    }
}

#[test]
fn loop_step_runs_under_bytecode() {
    let rt = runtime_with_eval("bytecode");
    for (src, expected) in cases() {
        let result = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("bytecode eval failed for `{src}`: {e}"));
        assert_eq!(result, expected, "bytecode mismatch for `{src}`");
    }
}

#[test]
fn loop_step_runs_under_cranelift() {
    let rt = runtime_with_eval("cranelift");
    for (src, expected) in cases() {
        let result = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("cranelift eval failed for `{src}`: {e}"));
        assert_eq!(result, expected, "cranelift mismatch for `{src}`");
    }
}

/// All three engines produce identical results for every case (cross-engine
/// parity, independent of the hardcoded `expected` values).
#[test]
fn loop_step_engine_parity() {
    let tw = runtime_with_eval("tree-walk");
    let bc = runtime_with_eval("bytecode");
    let cl = runtime_with_eval("cranelift");
    for (src, _expected) in cases() {
        let r_tw = tw.run(&Source::new(src)).expect("tree-walk");
        let r_bc = bc.run(&Source::new(src)).expect("bytecode");
        let r_cl = cl.run(&Source::new(src)).expect("cranelift");
        assert_eq!(r_tw, r_bc, "tree-walk != bytecode for `{src}`");
        assert_eq!(r_tw, r_cl, "tree-walk != cranelift for `{src}`");
    }
}
