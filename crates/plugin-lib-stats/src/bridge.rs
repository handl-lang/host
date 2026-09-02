//! Handl-authored `stats` bridge.
//!
//! The stats *logic* (sum, avg, product, min, max, median, stddev) is
//! defined once in handl source (`kit-stdlib/data/stats/stats.hnl`) as a
//! record of arrows; this module is the thin Rust loader/bridge shared by
//! every `Function` in [`crate::module`] and the platform plugin. It
//! `include_str!`s the data file, evaluates it with a minimal runtime that
//! carries `MathModule` (for `math.sqrt` in `stddev`) and `TypeModule` (for
//! `type.is` non-number filtering), and applies the requested field to the
//! caller's array argument.
//!
//! Per the `hello` migration's key constraint, a handl closure `Value`
//! produced by one bridge runtime cannot be cached and called later from an
//! outer runtime — the tree-walk engine indexes into a thread-local
//! `CURRENT_AST` arena. So the minimal `Runtime` is cached per-thread, and
//! only the small applied expression `(stats_src).FUNC(INPUT)` is
//! lexed/parsed/evaluated per call.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::Source;
use handl_platform::{EvalError, ModuleLookup};
use handl_plugin_lib_utils::{MathModule, TypeModule};
use handl_plugin_module::ModuleRegistry;
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};

/// The handl source defining the stats record — the single source of truth
/// for the stats logic. Loaded at compile time from the kit-stdlib data dir.
const STATS_HNL: &str = include_str!("../../../kit-stdlib/data/stats/stats.hnl");

thread_local! {
    /// A minimal runtime for evaluating `stats.hnl`. Cached per-thread
    /// because the tree-walk engine stores the active AST / `Language` in
    /// thread-locals. The module registry wires `MathModule` (for
    /// `math.sqrt` used by `stddev`) and `TypeModule` (for `type.type_of`
    /// used to filter non-numeric elements, matching the prior Rust
    /// `filter_map(|v| v.as_f64())` semantics).
    static STATS_RUNTIME: RefCell<Option<Runtime>> = RefCell::new(None);
}

/// Build the minimal runtime used to evaluate the handl-authored stats.
///
/// Reuses the canonical `Language` (assembled via
/// `handl_kit_data_canonical`) bound to the data lexer, Pratt parser, and
/// tree-walk evaluator — but with a module registry containing only
/// `MathModule` and `TypeModule` (no globals, no meta chain).
fn build_stats_runtime() -> Result<Runtime, handl_language::LangError> {
    let kit = handl_kit_data_canonical::assemble()?;
    let lang = kit.lang().clone();
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval("tree-walk");
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new()),
    );
    let registry = Rc::new(registry);

    // Module registry: math.sqrt (stddev) + type.is (non-number filtering).
    let mut modules = ModuleRegistry::new();
    modules.register(MathModule);
    modules.register(TypeModule);
    let modules: Rc<dyn ModuleLookup> = Rc::new(modules);

    Runtime::from_registry_with_globals(lang, &registry, &config, Some(modules), None)
}

/// Strip `--` comment lines from `stats.hnl` so the embedded source parses
/// as a bare expression. The data file's header comment documents the
/// definitions but is not valid inside the applied `(stats_src).func(input)`
/// form the Pratt parser evaluates, so it is removed before evaluation.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialize a JSON array value into a handl array literal string.
///
/// Handl array/number/string/bool/null literals are JSON-compatible, so
/// `serde_json::to_string` produces valid handl source for the number
/// arrays this module accepts.
fn array_to_handl_literal(arr: &serde_json::Value) -> Result<String, EvalError> {
    if !arr.is_array() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array"
        )));
    }
    Ok(serde_json::to_string(arr)
        .map_err(|e| EvalError::Other(format!("stats: failed to serialize input: {e}")))?)
}

/// Evaluate the handl-authored stats function `func` applied to `input`
/// (a JSON array) and return the resulting JSON value.
///
/// Reuses a cached per-thread minimal runtime; only the small applied
/// expression `(stats_src).func(input)` is lexed/parsed/evaluated per call.
pub(crate) fn stats_via_handl(
    func: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    let arr_literal = array_to_handl_literal(input)?;
    let stats_src = strip_comment_lines(STATS_HNL);
    let source = format!("({stats_src}).{func}({arr_literal})");
    STATS_RUNTIME.with(|cell| {
        let mut slot = cell.borrow_mut();
        let runtime = slot.get_or_insert_with(|| {
            build_stats_runtime().expect("canonical kit assembly should not fail")
        });
        runtime
            .run(&Source::new(&source))
            .map_err(|e| EvalError::Other(format!("stats.{func}: {e}")))
    })
}
