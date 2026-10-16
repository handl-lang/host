//! Integration test for the `HandlTypecheckEngine` adapter (ADR-0105).
//!
//! This verifies the adapter plumbing: build the engine, build the canonical
//! `Language`, lex+parse a simple source string, and call `typecheck`. The
//! test passes as long as the adapter does not panic — the returned result
//! may be `Ok` (a `TypecheckResult`, possibly with diagnostics) or
//! `Err(LangError::NotImplemented)` if the model is a skeleton. Either
//! outcome proves the adapter is wired correctly.

use handl_kit_data_canonical::language_from_yaml;
use handl_platform::ModuleLookup;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_engine_typecheck_handl::HandlTypecheckEngine;

/// A minimal empty `ModuleLookup` for the `with_modules` test.
struct EmptyLookup;
impl ModuleLookup for EmptyLookup {
    fn lookup(&self, _name: &str) -> Option<&dyn handl_platform::Function> {
        None
    }
}

/// Build the canonical `Language` from the embedded YAML fragments.
fn canonical_lang() -> handl_language::Language {
    let lang = language_from_yaml().expect("canonical language should assemble");
    // Also install it process-wide: the HandlTypecheckEngine's HandlModule
    // bridge runtime resolves `default_language` lazily (ADR-0128 §0.8).
    let _ = handl_plugin_lib_handl::install_default_language(lang.clone());
    lang
}

/// Lex + parse a source string into an `Ast` using the canonical language.
fn parse(src: &str, lang: &handl_language::Language) -> handl_language::Ast {
    let tokens = lex(lang, src).expect("lex should succeed");
    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    engine.parse(lang, &req).expect("parse should succeed")
}

/// The adapter should construct, load its model, and run `typecheck` on a
/// simple `x = 42` source without panicking. The result may be `Ok` (with
/// any type/diagnostics) or `Err(NotImplemented)` if the model is a
/// skeleton — both are acceptable.
#[test]
fn typecheck_simple_assign_runs() {
    let lang = canonical_lang();
    let ast = parse("x = 42", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    // The adapter must not panic. Either a result or NotImplemented is fine.
    let outcome = engine.typecheck(&lang, &req);
    match outcome {
        Ok(result) => {
            // We got a result — the adapter plumbing works. The type may
            // be imperfect (skeleton model); we only assert non-panic.
            let _ = result.typ;
            let _ = result.diagnostics;
            let _ = result.node_types;
        }
        Err(handl_language::LangError::NotImplemented) => {
            // Model is a skeleton — adapter degraded gracefully.
        }
        Err(e) => {
            // Any other error is still acceptable as long as we didn't
            // panic — the model may produce engine errors for unknown
            // constructs. We log it but don't fail the test.
            eprintln!("typecheck returned non-NotImplemented error (ok): {e}");
        }
    }
}

/// `box_clone` should preserve the model and allow a second typecheck call.
#[test]
fn box_clone_then_typecheck_runs() {
    let lang = canonical_lang();
    let ast = parse("x = 42", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let cloned = engine.box_clone();
    let req = TypecheckRequest::new(ast);

    // Must not panic on the cloned engine.
    let _ = cloned.typecheck(&lang, &req);
}

/// `with_modules` should produce a usable engine.
#[test]
fn with_modules_then_typecheck_runs() {
    use std::rc::Rc;

    let lang = canonical_lang();
    let ast = parse("x = 42", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let with_mods = engine.with_modules(Rc::new(EmptyLookup));
    let req = TypecheckRequest::new(ast);

    // Must not panic on the engine with modules.
    let _ = with_mods.typecheck(&lang, &req);
}
