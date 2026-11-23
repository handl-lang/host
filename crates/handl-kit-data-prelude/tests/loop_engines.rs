//! End-to-end `while` tests across all three eval engines
//! (tree-walk, bytecode, cranelift).
//!
//! ADR-0074 replaces the old `loop`/`returnA`/`then` surface syntax with
//! v0.1 `while` syntax. The example `.hnl` files use `while { state } { body }`
//! with `return v` (exit) / `{ state: v }` (continue). The `then` keyword is
//! removed from `if` expressions.
//!
//! The `while` node kind uses the kernel `Loop` combinator via `loop_step`
//! (ADR-0052) — `loop_step` accepts bare values (from `return v`) as exit
//! signals and `{ state: v }` as continue. `{ out: v }` is accepted for
//! backward compatibility with the kernel `Loop` step protocol.
//!
//! NOTE: ADR-0074 defines `while cond { body }` as sugar over `rec` with
//! implicit feedback. Full `rec` feedback semantics require runtime support
//! (task 4). Until then, the eval uses the state-passing `loop_step` protocol:
//! child 0 is the initial state object and child 1 is a block containing the
//! body closure. The examples use this form.

use handl_kit_data::{DataKit, DataKitManifest};
use handl_kit_data_prelude;
use handl_language::Source;
use hnl_engine_exec_bytecode::BytecodeEngine;
use hnl_engine_exec_cranelift::CraneliftEngine;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, RuntimeConfig};
use serde_json::json;
use std::rc::Rc;

/// A canonical-kit manifest that selects `eval` as its eval engine.
fn manifest_with_eval(eval: &str) -> DataKitManifest {
    DataKitManifest::new().with_runtime(
        RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval(eval),
    )
}

/// Build a runtime over the full canonical language with the requested eval
/// engine bound. The module chain is `None`.
fn runtime_with_eval(eval: &str) -> handl_runtime::Runtime {
    let reg = handl_kit_data_prelude::registry();
    let mut registry = EngineRegistry::new();
    registry.register_lex("data", Box::new(DataLexerEngine::new()));
    registry.register_parse("pratt", PrattEngine::new());
    registry.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    registry.register_eval("bytecode", Box::new(BytecodeEngine::new()));
    registry.register_eval("cranelift", Box::new(CraneliftEngine::new()));
    let registry = Rc::new(registry);
    let base = handl_kit_data_prelude::language_from_yaml().expect("canonical language");
    let kit = DataKit::assemble(&manifest_with_eval(eval), &reg, base)
        .expect("canonical kit assembles");
    kit.into_runtime(&registry, None)
        .expect("runtime binds engines")
}

/// `(source, expected)` cases — these use v0.1 `while { state } { body }`
/// syntax from the migrated example files. The `while` node uses the kernel
/// `Loop` combinator via `loop_step` with `return v` (exit) / `{ state: v }`
/// (continue).
fn cases() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        // factorial.hnl — while with destructured accumulator.
        (
            include_str!("../../../examples/handl/factorial.hnl"),
            json!(120),
        ),
        // fibonacci.hnl — while with 3 destructured vars.
        (
            include_str!("../../../examples/handl/fibonacci.hnl"),
            json!(55),
        ),
        // sum-to-n.hnl — while destructured accumulator.
        (
            include_str!("../../../examples/handl/sum-to-n.hnl"),
            json!(5050),
        ),
        // gcd.hnl — Euclidean GCD with destructured params.
        (include_str!("../../../examples/handl/gcd.hnl"), json!(12)),
        // power.hnl — while with 3 destructured vars, 2^10.
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

/// All three engines agree with the expected result for every `while` case.
#[test]
fn while_step_runs_under_tree_walk() {
    let rt = runtime_with_eval("tree-walk");
    for (src, expected) in cases() {
        let result = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("tree-walk eval failed for `{src}`: {e}"));
        assert_eq!(result, expected, "tree-walk mismatch for `{src}`");
    }
}

/// Bytecode engine agrees with the expected result for every `while` case.
#[test]
fn while_step_runs_under_bytecode() {
    let rt = runtime_with_eval("bytecode");
    for (src, expected) in cases() {
        let result = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("bytecode eval failed for `{src}`: {e}"));
        assert_eq!(result, expected, "bytecode mismatch for `{src}`");
    }
}

/// Cranelift engine agrees with the expected result for every `while` case.
#[test]
fn while_step_runs_under_cranelift() {
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
fn while_step_engine_parity() {
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
