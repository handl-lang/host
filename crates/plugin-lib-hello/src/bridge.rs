//! Handl-authored `greet` bridge.
//!
//! The greet *logic* is defined once in handl source
//! (`kit-stdlib/data/hello/greet.hnl`); this module is the thin Rust
//! loader/bridge shared by `HelloGreet` (the runtime `Function`) and
//! `HelloPlugin` (the platform capability). It `include_str!`s the data
//! file, evaluates it with a minimal module-free runtime (greet uses only
//! the native `+` string-concat op, so no `ModuleLookup` is needed), and
//! applies the resulting closure to the caller's argument.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::Source;
use handl_platform::EvalError;
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};

/// The handl source defining `greet` — the single source of truth for the
/// greeting logic. Loaded at compile time from the kit-stdlib data dir.
const GREET_HNL: &str = include_str!("../../../kit-stdlib/data/hello/greet.hnl");

thread_local! {
    /// A minimal, module-free runtime for evaluating `greet.hnl`. Cached
    /// per-thread because the tree-walk engine stores the active AST /
    /// `Language` in thread-locals. No module registry is wired: `greet`
    /// uses only the native `+` string-concatenation op.
    static GREET_RUNTIME: RefCell<Option<Runtime>> = RefCell::new(None);
}

/// Build the minimal runtime used to evaluate the handl-authored `greet`.
///
/// Reuses the canonical `Language` (assembled via
/// `handl_kit_data_canonical`) bound to the data lexer, Pratt parser, and
/// tree-walk evaluator — but with no module registry and no globals.
fn build_greet_runtime() -> Result<Runtime, handl_language::LangError> {
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
    Runtime::from_registry(lang, &registry, &config)
}

/// Escape a Rust string for embedding inside a handl `"..."` string literal.
///
/// Only `"` and `\` need escaping: the delimited lexer consumes any other
/// character (including newlines) raw, and the Pratt parser's string-literal
/// unescaping only rewrites the four canonical escapes (`\\`, `\"`, `\n`,
/// `\t`), so all other characters pass through faithfully.
fn escape_handl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

/// Strip `//` comment lines from `greet.hnl` so the embedded source parses
/// as a bare expression. The data file's header comment documents the
/// definition but is not valid inside the applied `(greet)(name)` form the
/// Pratt parser evaluates, so it is removed before evaluation.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Evaluate the handl-authored `greet` applied to `name` and return the
/// resulting greeting string.
///
/// Reuses a cached per-thread minimal runtime; only the small applied
/// expression `(greet.hnl)(name)` is lexed/parsed/evaluated per call.
pub(crate) fn greet_via_handl(name: &str) -> Result<String, EvalError> {
    let escaped = escape_handl_string(name);
    let greet_src = strip_comment_lines(GREET_HNL);
    let source = format!("({greet_src})(\"{escaped}\")");
    GREET_RUNTIME.with(|cell| {
        let mut slot = cell.borrow_mut();
        let runtime = slot.get_or_insert_with(|| {
            build_greet_runtime().expect("canonical kit assembly should not fail")
        });
        runtime
            .run(&Source::new(&source))
            .map_err(|e| EvalError::Other(format!("hello.greet: {e}")))
            .and_then(|v| {
                v.as_str().map(String::from).ok_or_else(|| {
                    EvalError::Other("hello.greet: greet did not return a string".to_string())
                })
            })
    })
}
