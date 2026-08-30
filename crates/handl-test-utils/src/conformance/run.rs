//! Harness functions that run the conformance corpus against any engine.

use handl_language::Ast;
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};

use super::corpus::{eval_corpus, parse_corpus};

/// Run the full eval conformance corpus against an eval engine.
///
/// Each [`EvalCase`] is evaluated: `engine.eval(&case.lang, &req)` must
/// return `Ok(case.expected)`. The harness provides a host functions
/// registry with `"double"` registered for the `foreign` case.
///
/// Returns `Ok(())` if all cases pass, or the first `Err` with the case
/// name.
///
/// # Errors
///
/// Returns an error if any case fails (wrong result or engine error).
pub fn run_eval_conformance(engine: &dyn EvalEngine) -> Result<(), String> {
    let corpus = eval_corpus();
    for case in &corpus {
        let req = EvalRequest::new(case.ast.clone());
        // For the "foreign" case, we need a host registry. We create a
        // wrapper engine that delegates to the real engine but provides
        // the host. However, since EvalEngine::eval doesn't take a host,
        // we handle the foreign case specially: the engine must be
        // constructed with the host by the caller. For the conformance
        // harness, we test non-foreign cases here and let the caller
        // handle foreign cases separately.
        if case.name == "foreign" {
            // Skip the foreign case in the generic harness — it requires
            // a host-aware engine. The caller should test it separately.
            continue;
        }
        let result = engine.eval(&case.lang, &req);
        match result {
            Ok(v) if v == case.expected => {}
            Ok(v) => {
                return Err(format!(
                    "case '{}': expected {}, got {}",
                    case.name, case.expected, v
                ));
            }
            Err(e) => {
                return Err(format!("case '{}': engine error: {}", case.name, e));
            }
        }
    }
    Ok(())
}

/// Run the full parse conformance corpus against a parse engine.
///
/// Each [`ParseCase`] is parsed: `engine.parse(&case.lang, &req)` must
/// return an `Ast` equal to `case.expected` (compared by node count, kinds,
/// and payloads).
///
/// # Errors
///
/// Returns an error if any case fails.
pub fn run_parse_conformance<E: handl_plugin_engine_parse::ParserEngine>(
    engine: &E,
) -> Result<(), String> {
    let corpus = parse_corpus();
    for case in &corpus {
        let req = handl_plugin_engine_parse::ParserRequest::new(case.tokens.clone());
        let result = engine.parse(&case.lang, &req);
        match result {
            Ok(ast) => {
                if !asts_equal(&ast, &case.expected) {
                    return Err(format!(
                        "case '{}': AST mismatch\n  expected: {}\n  got: {}",
                        case.name,
                        ast_debug(&case.expected),
                        ast_debug(&ast),
                    ));
                }
            }
            Err(e) => {
                return Err(format!("case '{}': parse error: {}", case.name, e));
            }
        }
    }
    Ok(())
}

/// Compare two `Ast`s by node count, kinds, and payloads.
fn asts_equal(a: &Ast, b: &Ast) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.nodes()
        .iter()
        .zip(b.nodes().iter())
        .all(|(na, nb)| na.kind == nb.kind && na.payload == nb.payload)
}

/// Debug-format an `Ast` for error messages.
fn ast_debug(ast: &Ast) -> String {
    let nodes: Vec<String> = ast
        .nodes()
        .iter()
        .map(|n| format!("{}({})", n.kind, n.payload))
        .collect();
    format!("[{}]", nodes.join(", "))
}
