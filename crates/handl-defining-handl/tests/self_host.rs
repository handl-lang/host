//! # self_host.rs — ADR-0044 Phase 8 self-hosting test
//!
//! Per ADR-0044 Phase 8 testing: "load `handl-defining-handl` as a `Language`
//! value, `eval(handlLanguage, ast)` produces the same result as the
//! host-built handl for a corpus of programs."
//!
//! This test loads the self-describing `Language` (via both [`language`] and
//! [`language_from_data_file`]) and evaluates the self-hosting corpus under
//! all three eval engines (tree-walk, bytecode, cranelift). All engines must
//! agree with the expected results — proving the self-describing language can
//! evaluate its own constructs.
//!
//! ## Scope
//!
//! The corpus uses only the migrated subset (identifier, value, arithmetic,
//! let, call, branch, function). Full handl self-hosting is deferred until
//! the remaining ~43 constructs are migrated to the data-driven model. This
//! test proves the **mechanism**: a `Language` value can describe a language
//! and be evaluated under itself.

use handl_defining_handl::{corpus, language, language_from_data_file};
use handl_language::LangError;
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_bytecode::BytecodeEngine;
use handl_plugin_engine_eval_cranelift::CraneliftEngine;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_runtime::{Runtime, RuntimeConfig};

/// Evaluate a corpus case under a `Runtime` (tree-walk or bytecode, selected
/// via `RuntimeConfig`) and return the result.
fn eval_with_runtime(
    lang: &handl_language::Language,
    config: &RuntimeConfig,
    ast: &handl_language::Ast,
) -> Result<handl_language::Value, LangError> {
    let rt = Runtime::from_config(lang.clone(), config, None)?;
    rt.eval(ast)
}

/// The emitter-assembled self-describing language evaluates the corpus
/// correctly under the tree-walk engine (the reference interpreter).
#[test]
fn self_host_tree_walk_emitter_language() {
    let lang = language();
    let config = RuntimeConfig::new().with_parse("pratt").with_eval("tree-walk");
    for case in corpus() {
        let result = eval_with_runtime(&lang, &config, &case.ast);
        assert_eq!(
            result.unwrap(),
            case.expected,
            "tree-walk / emitter-lang: case '{}' ({}) failed",
            case.name,
            case.source
        );
    }
}

/// The data-file-loaded self-describing language evaluates the corpus
/// correctly under the tree-walk engine.
///
/// This is the ADR §6 end state: the language is loaded from the authored
/// `data/handl.json` file — no Rust emitters needed at load time — and
/// evaluates its own constructs.
#[test]
fn self_host_tree_walk_data_file_language() {
    let lang = language_from_data_file().expect("data file must load");
    let config = RuntimeConfig::new().with_parse("pratt").with_eval("tree-walk");
    for case in corpus() {
        let result = eval_with_runtime(&lang, &config, &case.ast);
        assert_eq!(
            result.unwrap(),
            case.expected,
            "tree-walk / data-file-lang: case '{}' ({}) failed",
            case.name,
            case.source
        );
    }
}

/// The self-describing language evaluates the corpus correctly under the
/// bytecode engine.
#[test]
fn self_host_bytecode_emitter_language() {
    let lang = language();
    let engine = BytecodeEngine::new();
    for case in corpus() {
        let req = EvalRequest::new(case.ast.clone());
        let result = engine.eval(&lang, &req);
        assert_eq!(
            result.unwrap(),
            case.expected,
            "bytecode / emitter-lang: case '{}' ({}) failed",
            case.name,
            case.source
        );
    }
}

/// The self-describing language evaluates the corpus correctly under the
/// cranelift (native) engine.
#[test]
fn self_host_cranelift_emitter_language() {
    let lang = language();
    let engine = CraneliftEngine::new();
    for case in corpus() {
        let req = EvalRequest::new(case.ast.clone());
        let result = engine.eval(&lang, &req);
        assert_eq!(
            result.unwrap(),
            case.expected,
            "cranelift / emitter-lang: case '{}' ({}) failed",
            case.name,
            case.source
        );
    }
}

/// The data-file-loaded language evaluates the corpus correctly under the
/// cranelift engine — the data-file path compiled to native.
#[test]
fn self_host_cranelift_data_file_language() {
    let lang = language_from_data_file().expect("data file must load");
    let engine = CraneliftEngine::new();
    for case in corpus() {
        let req = EvalRequest::new(case.ast.clone());
        let result = engine.eval(&lang, &req);
        assert_eq!(
            result.unwrap(),
            case.expected,
            "cranelift / data-file-lang: case '{}' ({}) failed",
            case.name,
            case.source
        );
    }
}

/// All three eval engines agree on the self-hosted corpus (emitter language).
///
/// This is the cross-engine conformance gate scoped to the self-hosting
/// corpus: tree-walk, bytecode, and cranelift must produce identical results
/// for every subset program.
#[test]
fn all_three_engines_agree_emitter_language() {
    let lang = language();
    let tw = TreeWalkEngine::new();
    let bc = BytecodeEngine::new();
    let cl = CraneliftEngine::new();
    for case in corpus() {
        let req = EvalRequest::new(case.ast.clone());
        let tw_res = tw.eval(&lang, &req);
        let bc_res = bc.eval(&lang, &req);
        let cl_res = cl.eval(&lang, &req);
        assert_eq!(
            tw_res, bc_res,
            "case '{}': tree-walk and bytecode diverged",
            case.name
        );
        assert_eq!(
            tw_res, cl_res,
            "case '{}': tree-walk and cranelift diverged",
            case.name
        );
        assert_eq!(
            tw_res.unwrap(),
            case.expected,
            "case '{}': result does not match expected",
            case.name
        );
    }
}

/// All three eval engines agree on the self-hosted corpus (data-file
/// language).
#[test]
fn all_three_engines_agree_data_file_language() {
    let lang = language_from_data_file().expect("data file must load");
    let tw = TreeWalkEngine::new();
    let bc = BytecodeEngine::new();
    let cl = CraneliftEngine::new();
    for case in corpus() {
        let req = EvalRequest::new(case.ast.clone());
        let tw_res = tw.eval(&lang, &req);
        let bc_res = bc.eval(&lang, &req);
        let cl_res = cl.eval(&lang, &req);
        assert_eq!(
            tw_res, bc_res,
            "case '{}': tree-walk and bytecode diverged (data-file lang)",
            case.name
        );
        assert_eq!(
            tw_res, cl_res,
            "case '{}': tree-walk and cranelift diverged (data-file lang)",
            case.name
        );
        assert_eq!(
            tw_res.unwrap(),
            case.expected,
            "case '{}': result does not match expected (data-file lang)",
            case.name
        );
    }
}
