//! Tests for the [`NbeTypecheckEngine`].
//!
//! These tests construct a minimal [`Language`] with type rules and verify
//! the judgment ops, combinators, and engine lifecycle methods end-to-end.

use super::*;
use handl_language::fragments::{EvalSchemeDef, TypeRuleDef};
use handl_language::{Ast, AstNode, Language, Scheme, Value};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;
use std::rc::Rc;

/// Build an [`NbeTypecheckEngine`] backed by a fresh [`TreeWalkEngine`].
fn engine() -> NbeTypecheckEngine {
    NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()))
}

/// The expected tagged type value for `type.number` in `handl.canonical`.
fn type_number() -> Value {
    json!({"__tag__":"type.number","__registry__":"handl.canonical"})
}

/// The expected tagged type value for `type.string` in `handl.canonical`.
fn type_string() -> Value {
    json!({"__tag__":"type.string","__registry__":"handl.canonical"})
}

// ---------------------------------------------------------------------------
// a. empty ast errors
// ---------------------------------------------------------------------------

#[test]
fn typecheck_empty_ast_errors() {
    let engine = engine();
    let lang = Language::new();
    let req = TypecheckRequest::new(Ast::new());
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("empty ast")),
        "expected empty-ast error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// b. no type rule errors
// ---------------------------------------------------------------------------

#[test]
fn typecheck_no_rule_errors() {
    let engine = engine();
    let lang = Language::new();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("no type rule for kind 'lit'")),
        "expected no-rule error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// c. lit synth → type.number
// ---------------------------------------------------------------------------

#[test]
fn typecheck_lit_synth_type_number() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// d. var lookup from Γ
// ---------------------------------------------------------------------------

#[test]
fn typecheck_var_lookup() {
    let engine = engine();
    let mut lang = Language::new();
    // The var rule: extend Γ with "x" → type.number, then lookup "x".
    // We use a Seq: construct(type.number) >>> extend("x") >>> lookup("x").
    // But extend needs the type as incoming, so:
    //   construct(type.number) >>> extend("x") >>> lookup("x")
    lang.types
        .merge(TypeRuleDef::new(
            "type.var",
            "var",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::seq(
                    Scheme::arr_judgment_lit("extend", vec![], Value::from("x")),
                    Scheme::arr_judgment_lit("lookup", vec![], Value::from("x")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x")));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// e. Seq threading: synth_child(0) >>> bind("t") >>> result("t")
// ---------------------------------------------------------------------------

#[test]
fn typecheck_seq_threading() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule for "lit": synth = construct(type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // Rule for "wrap": synth = synth_child(0) >>> bind("t") >>> result("t")
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrap",
            "wrap",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("t")),
                    Scheme::arr_judgment_lit("result", vec![], Value::from("t")),
                ),
            ),
        ))
        .unwrap();
    // AST: [lit(42), wrap{children:[0]}]  — root is wrap (last node)
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(
        AstNode::new("wrap").with_payload(json!({"children": [0]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// f. unify success (equal values)
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unify_success() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> unify(lit=type.number) >>> construct(type.string)
    // The unify should succeed (both type.number), and the result is type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::seq(
                    Scheme::arr_judgment_lit("unify", vec![], type_number()),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// g. unify failure (different values)
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unify_failure() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> unify(lit=type.string) → fails
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::arr_judgment_lit("unify", vec![], type_string()),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// h. fresh_meta + solve_meta + unify solves
// ---------------------------------------------------------------------------

#[test]
fn typecheck_fresh_meta_and_solve() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule:
    //   fresh_meta >>> bind("m")
    //   >>> solve_meta(lit=0, args=[result("m")])  -- wait, solve_meta needs id + val
    // Simpler: fresh_meta >>> bind("m") >>> unify(lit=type.number) >>> result("m")
    // The unify solves the meta to type.number, then result("m") returns the solved value.
    // But result("m") returns the *stored* value which is the unsolved meta.
    // We need to re-read the meta after solving. Use a different approach:
    //   fresh_meta >>> bind("m") >>> solve_meta(id=0 via lit, val=type.number via args)
    //   >>> lookup_meta... we don't have lookup_meta as a judgment op.
    //
    // Instead, test that unify with a meta solves it:
    //   fresh_meta >>> unify(lit=type.number) >>> construct(type.string)
    // The unify solves the fresh meta to type.number (no error). Result is type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("fresh_meta", vec![], Value::Null),
                Scheme::seq(
                    Scheme::arr_judgment_lit("unify", vec![], type_number()),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// i. box_clone produces a working engine
// ---------------------------------------------------------------------------

#[test]
fn typecheck_box_clone() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);

    let cloned: Box<dyn TypecheckEngine> = engine.box_clone();
    let result = cloned.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// j. with_modules produces a working engine
// ---------------------------------------------------------------------------

#[test]
fn typecheck_with_modules() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);

    let modules: Rc<dyn handl_platform::ModuleLookup> = Rc::new(ModuleRegistry::new());
    let with_mods: Box<dyn TypecheckEngine> = engine.with_modules(modules);
    let result = with_mods.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// Extra: construct with payload fields
// ---------------------------------------------------------------------------

#[test]
fn typecheck_construct_with_payload() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct("type.pi", [const("x") >>> construct(type.number)])
    // The arg produces {"param": type.number} → merged into the type value.
    // Actually construct args should produce objects whose keys become fields.
    // Use: construct("type.pi", [const({"param": type.number})])
    // But const returns lit directly. So lit = {"param": type.number}.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit(
                "construct",
                vec![Scheme::arr_native_lit(
                    "const",
                    vec![],
                    json!({"param": type_number()}),
                )],
                Value::from("type.pi"),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({
            "__tag__": "type.pi",
            "__registry__": "handl.canonical",
            "param": type_number()
        })
    );
}

// ---------------------------------------------------------------------------
// Extra: check_node via synth-then-unify fallback
// ---------------------------------------------------------------------------

#[test]
fn typecheck_check_node_fallback() {
    let engine = engine();
    let mut lang = Language::new();
    // lit rule: synth = construct(type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // wrap rule: check_child(0, construct(type.number))
    //   — lit has no check scheme, so check_node falls back to synth-then-unify.
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.wrap",
                "wrap",
                // synth: just return type.number (not used in this test path,
                // but required for the rule to be valid)
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            )
            .with_check(Scheme::arr_judgment_lit(
                "check_child",
                vec![Scheme::arr_judgment_lit(
                    "construct",
                    vec![],
                    Value::from("type.number"),
                )],
                Value::from(0u64),
            )),
        )
        .unwrap();
    // AST: [lit(42), wrap{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(AstNode::new("wrap").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    // The root is "wrap" — its synth returns type.number.
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// Extra: with_registry_id builder
// ---------------------------------------------------------------------------

#[test]
fn typecheck_with_registry_id() {
    let engine =
        NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new())).with_registry_id("custom");
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({"__tag__":"type.number","__registry__":"custom"})
    );
}

// ---------------------------------------------------------------------------
// Extra: node_payload judgment op
// ---------------------------------------------------------------------------

#[test]
fn typecheck_node_payload() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: node_payload >>> match_type(cases)
    // The node_payload returns the literal value; match_type dispatches on it.
    // But node_payload returns a raw value, not a type value. match_type
    // requires a type value. So instead, just test node_payload returns
    // the payload by using it as the synth result directly.
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment("node_payload", vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!({"value": 42})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, json!({"value": 42}));
}

// ---------------------------------------------------------------------------
// Extra: lookup_var by index
// ---------------------------------------------------------------------------

#[test]
fn typecheck_lookup_var_by_index() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> extend("x") >>> lookup_var(0)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::seq(
                    Scheme::arr_judgment_lit("extend", vec![], Value::from("x")),
                    Scheme::arr_judgment_lit("lookup_var", vec![], Value::from(0u64)),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// Extra: unify with two args
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unify_two_args() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: unify(construct(type.number), construct(type.number)) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.number"),
                        ),
                        Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.number"),
                        ),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// Extra: unknown judgment op errors
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unknown_judgment_op_errors() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment("bogus_op", vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("unknown judgment op 'bogus_op'")),
        "expected unknown judgment op error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// Extra: native op other than const/id errors
// ---------------------------------------------------------------------------

#[test]
fn typecheck_unsupported_native_op_errors() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_native("add", vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("not supported in type rules yet")),
        "expected unsupported native op error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// Extra: const and id native ops work
// ---------------------------------------------------------------------------

#[test]
fn typecheck_const_and_id_native_ops() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: const(42) >>> id  → returns 42
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::from(42)),
                Scheme::arr_native("id", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(42));
}

// ---------------------------------------------------------------------------
// Extra: Choice combinator
// ---------------------------------------------------------------------------

#[test]
fn typecheck_choice_combinator() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct a choice value, then Choice(l=construct(type.number), r=construct(type.string))
    // Incoming = {"__choice__": "left", "value": null}
    // const({"__choice__": "left", "value": null}) >>> Choice(construct(type.number), construct(type.string))
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit(
                    "const",
                    vec![],
                    json!({"__choice__": "left", "value": Value::Null}),
                ),
                Scheme::choice(
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// Extra: First combinator
// ---------------------------------------------------------------------------

#[test]
fn typecheck_first_combinator() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: const([type.number, type.string]) >>> First(construct(type.bool))
    // First runs construct(type.bool) on the first element (type.number, ignored),
    // and carries type.string through → [type.bool, type.string]
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit(
                    "const",
                    vec![],
                    Value::Array(vec![type_number(), type_string()]),
                ),
                Scheme::first(Scheme::arr_judgment_lit(
                    "construct",
                    vec![],
                    Value::from("type.bool"),
                )),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    let expected_bool = json!({"__tag__":"type.bool","__registry__":"handl.canonical"});
    assert_eq!(result.typ, Value::Array(vec![expected_bool, type_string()]));
}

// ---------------------------------------------------------------------------
// Extra: Loop and App return NotImplemented
// ---------------------------------------------------------------------------

#[test]
fn typecheck_loop_not_implemented() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::loop_feedback(Scheme::arr_judgment("id", vec![])),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

#[test]
fn typecheck_app_not_implemented() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::app(Scheme::arr_judgment("id", vec![]), vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

// ---------------------------------------------------------------------------
// Extra: match_type dispatches on type tag
// ---------------------------------------------------------------------------

#[test]
fn typecheck_match_type_dispatches() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> match_type(args=[id, const(cases)])
    // cases = {"type.number": type_string(), "type.string": type_number()}
    // Matching type.number → returns type_string()
    let cases = json!({
        "type.number": type_string(),
        "type.string": type_number()
    });
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::arr_judgment(
                    "match_type",
                    vec![
                        Scheme::arr_native("id", vec![]),
                        Scheme::arr_native_lit("const", vec![], cases),
                    ],
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// Extra: eval_child delegates to the bound eval engine
// ---------------------------------------------------------------------------

#[test]
fn typecheck_eval_child_delegates_to_eval() {
    let engine = engine();
    let mut lang = Language::new();
    // lit eval rule: no scheme → tree-walk returns payload directly.
    // lit type rule: synth = construct(type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // wrap type rule: synth = eval_child(0) >>> construct(type.string)
    // eval_child(0) evaluates the lit child via the eval engine → returns 42.
    // Then construct(type.string) ignores incoming and returns type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrap",
            "wrap",
            Scheme::seq(
                Scheme::arr_judgment_lit("eval_child", vec![], Value::from(0u64)),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [lit(42), wrap{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(AstNode::new("wrap").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// Extra: lookup_meta helper on TypecheckContext (internal sanity)
// ---------------------------------------------------------------------------

#[test]
fn typecheck_context_meta_store() {
    let mut ctx = TypecheckContext::new("handl.canonical");
    let id = ctx.fresh_meta();
    assert_eq!(id, 0);
    assert!(ctx.lookup_meta(id).is_none());
    ctx.solve_meta(id, type_number());
    assert_eq!(ctx.lookup_meta(id), Some(&type_number()));
}

// ---------------------------------------------------------------------------
// Extra: lookup helper on TypecheckContext (internal sanity)
// ---------------------------------------------------------------------------

#[test]
fn typecheck_context_gamma_lookup() {
    let mut ctx = TypecheckContext::new("handl.canonical");
    ctx.extend("x".into(), type_number());
    ctx.extend("y".into(), type_string());
    assert_eq!(ctx.lookup("x"), Some(&type_number()));
    assert_eq!(ctx.lookup("y"), Some(&type_string()));
    assert!(ctx.lookup("z").is_none());
    // Reverse-order: extend "x" again shadows the first.
    ctx.extend("x".into(), type_string());
    assert_eq!(ctx.lookup("x"), Some(&type_string()));
}

// ===========================================================================
// ADR-0054 purity enforcement
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Foreign op in a type rule's synth scheme is rejected (unconditionally).
// ---------------------------------------------------------------------------

#[test]
fn foreign_op_in_type_rule_rejected() {
    let engine = engine();
    let mut lang = Language::new();
    // A type rule that directly authors a Foreign op in its synth scheme.
    lang.types
        .merge(TypeRuleDef::new(
            "type.io",
            "io",
            Scheme::arr_foreign("fs.read", vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("io"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("foreign op")
            && msg.contains("fs.read")
            && msg.contains("pure-eval")),
        "expected foreign-op-in-type-rule rejection, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 2. Native op (const) in a type rule is allowed.
// ---------------------------------------------------------------------------

#[test]
fn native_op_in_type_rule_allowed() {
    let engine = engine();
    let mut lang = Language::new();
    // const(42) is a pure native op — allowed in type rules.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_native_lit("const", vec![], Value::from(42)),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(42));
}

// ---------------------------------------------------------------------------
// 3. eval_child on a node whose eval scheme has no Foreign ops succeeds.
// ---------------------------------------------------------------------------

#[test]
fn eval_child_pure_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // lit eval scheme: pure native const returning the payload.
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.lit",
            "lit",
            Scheme::arr_native_lit("const", vec![], Value::from(42)),
        ))
        .unwrap();
    // lit type rule: synth = construct(type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // wrap type rule: synth = eval_child(0) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrap",
            "wrap",
            Scheme::seq(
                Scheme::arr_judgment_lit("eval_child", vec![], Value::from(0u64)),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [lit(42), wrap{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(AstNode::new("wrap").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 4. eval_child on a node whose eval scheme has a Foreign op is rejected.
// ---------------------------------------------------------------------------

#[test]
fn eval_child_with_foreign_rejected() {
    let engine = engine();
    let mut lang = Language::new();
    // io eval scheme: effectful Foreign op fs.read.
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.io",
            "io",
            Scheme::arr_foreign("fs.read", vec![]),
        ))
        .unwrap();
    // wrapper type rule: synth = eval_child(0) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("eval_child", vec![], Value::from(0u64)),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [io, wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("io"));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("effectful foreign ops")
            && msg.contains("fs.read")
            && msg.contains("pure-eval")),
        "expected effectful-foreign-ops rejection, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 5. with_pure(false) skips the purity check (the error must NOT be the
//    "effectful foreign ops" purity-check error).
// ---------------------------------------------------------------------------

#[test]
fn pure_flag_disabled_allows_foreign() {
    let engine =
        NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new())).with_pure(false);
    let mut lang = Language::new();
    // io eval scheme: effectful Foreign op fs.read.
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.io",
            "io",
            Scheme::arr_foreign("fs.read", vec![]),
        ))
        .unwrap();
    // wrapper type rule: synth = eval_child(0) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("eval_child", vec![], Value::from(0u64)),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [io, wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("io"));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    // With pure disabled, check_pure is skipped. The eval engine then runs
    // and fails because no module registry wires fs.read — but that failure
    // must NOT be the purity-check error. We only assert the purity check
    // was bypassed.
    let result = engine.typecheck(&lang, &req);
    match result {
        Ok(_) => { /* eval engine ran and succeeded — purity check skipped */ }
        Err(LangError::Engine(ref msg)) => {
            assert!(
                !msg.contains("effectful foreign ops"),
                "purity check should have been skipped (pure=false), but got: {}",
                msg
            );
        }
        Err(other) => {
            // Any non-Engine error is fine — the point is the purity check
            // (which raises an Engine error) was bypassed.
            let _ = other;
        }
    }
}

// ===========================================================================
// ADR-0054 richer type rules: json_type_construct, payload_field, field,
// extend-from-incoming, and end-to-end value/let/function rules.
// ===========================================================================

/// The expected tagged type value for `type.bool` in `handl.canonical`.
fn type_bool() -> Value {
    json!({"__tag__":"type.bool","__registry__":"handl.canonical"})
}

/// The expected tagged type value for `type.null` in `handl.canonical`.
fn type_null() -> Value {
    json!({"__tag__":"type.null","__registry__":"handl.canonical"})
}

// ---------------------------------------------------------------------------
// 1. json_type_construct dispatches on JSON type of incoming (number)
// ---------------------------------------------------------------------------

#[test]
fn json_type_construct_number() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: const(42) >>> json_type_construct → type.number
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::from(42)),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 2. json_type_construct dispatches on JSON type of incoming (string)
// ---------------------------------------------------------------------------

#[test]
fn json_type_construct_string() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::from("hello")),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 3. json_type_construct dispatches on JSON type of incoming (bool)
// ---------------------------------------------------------------------------

#[test]
fn json_type_construct_bool() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::from(true)),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_bool());
}

// ---------------------------------------------------------------------------
// 4. json_type_construct dispatches on JSON type of incoming (null)
// ---------------------------------------------------------------------------

#[test]
fn json_type_construct_null() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::Null),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_null());
}

// ---------------------------------------------------------------------------
// 5. payload_field extracts a field from the node's payload
// ---------------------------------------------------------------------------

#[test]
fn payload_field_extracts_name() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: payload_field("name") → returns the "name" field from payload.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(
        AstNode::new("test")
            .with_payload(json!({"name": "x", "children": [0, 1]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from("x"));
}

// ---------------------------------------------------------------------------
// 6. field builds a single-field object for construct args
// ---------------------------------------------------------------------------

#[test]
fn field_builds_single_field_object() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: field("param", [const(42)]) → {"param": 42}
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit(
                "field",
                vec![Scheme::arr_native_lit("const", vec![], Value::from(42))],
                Value::from("param"),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, json!({"param": 42}));
}

// ---------------------------------------------------------------------------
// 7. extend reads name from incoming, type from args[0]
// ---------------------------------------------------------------------------

#[test]
fn extend_from_incoming() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: const("x") >>> extend(args=[construct(type.number)]) >>> lookup("x")
    // extend has no lit → name from incoming ("x"), type from args[0].
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], Value::from("x")),
                Scheme::seq(
                    Scheme::arr_judgment(
                        "extend",
                        vec![Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.number"),
                        )],
                    ),
                    Scheme::arr_judgment_lit("lookup", vec![], Value::from("x")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 8. value rule dispatches on JSON type (number and string)
// ---------------------------------------------------------------------------

#[test]
fn value_rule_dispatches_on_json_type() {
    let engine = engine();
    let mut lang = Language::new();
    // The new value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();

    // Number literal → type.number
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());

    // String literal → type.string
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(json!("hello")));
    let req2 = TypecheckRequest::new(ast2);
    let result2 = engine.typecheck(&lang, &req2).unwrap();
    assert_eq!(result2.typ, type_string());
}

// ---------------------------------------------------------------------------
// 9. let rule extends Γ and synths body
// ---------------------------------------------------------------------------

#[test]
fn let_rule_extends_gamma_and_synths_body() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // var rule: node_payload >>> lookup (name from incoming)
    lang.types
        .merge(TypeRuleDef::new(
            "type.var",
            "var",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("lookup", vec![]),
            ),
        ))
        .unwrap();
    // let rule: synth_child(0) >>> bind("vartype")
    //   >>> payload_field("name") >>> extend(args=[result("vartype")])
    //   >>> synth_child(1)
    lang.types
        .merge(TypeRuleDef::new(
            "type.let",
            "let",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("vartype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
                        Scheme::seq(
                            Scheme::arr_judgment(
                                "extend",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("vartype"),
                                )],
                            ),
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();

    // AST: [value(42), var("x"), let{name:"x", children:[0,1]}]
    // Root is the let node (last in arena).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The body is var("x"), which looks up "x" in Γ → type.number.
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 10. function rule constructs type.function with param and body types
// ---------------------------------------------------------------------------

#[test]
fn function_rule_constructs_function_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // function rule: synth_child(0) >>> bind("paramtype")
    //   >>> synth_child(1) >>> bind("bodytype")
    //   >>> construct("type.function", [field("param", result("paramtype")),
    //                                   field("body", result("bodytype"))])
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("paramtype"),
                                        )],
                                        Value::from("param"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("bodytype"),
                                        )],
                                        Value::from("body"),
                                    ),
                                ],
                                Value::from("type.function"),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();

    // AST: [value(42), value("hello"), function{children:[0,1]}]
    // Root is the function node (last in arena).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(
        AstNode::new("function").with_payload(json!({"children": [0, 1]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({
            "__tag__": "type.function",
            "__registry__": "handl.canonical",
            "param": type_number(),
            "body": type_string()
        })
    );
}

// ===========================================================================
// ADR-0054 legacy migration: recursive structural unification, type_field,
// synth_last_child, call/block/typeof/type-annot rules, check rules.
// ===========================================================================

/// The expected tagged type value for `type.unit` in `handl.canonical`.
fn type_unit() -> Value {
    json!({"__tag__":"type.unit","__registry__":"handl.canonical"})
}

/// Build a `type.function` value with the given `param` and `body` types.
fn type_function(param: Value, body: Value) -> Value {
    json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": param,
        "body": body
    })
}

// ---------------------------------------------------------------------------
// 1. unify tagged type values: same tag, same fields → succeeds
// ---------------------------------------------------------------------------

#[test]
fn unify_tagged_type_values_same_tag_same_fields() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: unify(two identical type.function values) >>> construct(type.string)
    let fun_a = type_function(type_number(), type_string());
    let fun_b = type_function(type_number(), type_string());
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_native_lit("const", vec![], fun_a),
                        Scheme::arr_native_lit("const", vec![], fun_b),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 2. unify tagged type values: same tag, different fields → fails
// ---------------------------------------------------------------------------

#[test]
fn unify_tagged_type_values_same_tag_different_fields() {
    let engine = engine();
    let mut lang = Language::new();
    // fun_a has param=type.number, body=type.string
    // fun_b has param=type.string, body=type.number
    // The param fields differ (type.number vs type.string) → unify fails.
    let fun_a = type_function(type_number(), type_string());
    let fun_b = type_function(type_string(), type_number());
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_native_lit("const", vec![], fun_a),
                    Scheme::arr_native_lit("const", vec![], fun_b),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 3. unify tagged type values: different tags → fails
// ---------------------------------------------------------------------------

#[test]
fn unify_tagged_type_values_different_tags() {
    let engine = engine();
    let mut lang = Language::new();
    // type.number vs type.string — different tags, no recursive case.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_native_lit("const", vec![], type_number()),
                    Scheme::arr_native_lit("const", vec![], type_string()),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 4. unify recursive nested type values → succeeds
// ---------------------------------------------------------------------------

#[test]
fn unify_recursive_nested_type_values() {
    let engine = engine();
    let mut lang = Language::new();
    // type.function{param=type.function{param=type.number, body=type.string},
    //                body=type.number}
    // Both sides identical → recursive unification succeeds.
    let inner = type_function(type_number(), type_string());
    let outer_a = type_function(inner.clone(), type_number());
    let outer_b = type_function(inner, type_number());
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_native_lit("const", vec![], outer_a),
                        Scheme::arr_native_lit("const", vec![], outer_b),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 5. type_field extracts a field from a type value
// ---------------------------------------------------------------------------

#[test]
fn type_field_extracts_from_type_value() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: type_field("param", [construct("type.function",
    //   [field("param", construct(type.number)),
    //    field("body", construct(type.string))])])
    // → returns type.number
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit(
                "type_field",
                vec![Scheme::arr_judgment_lit(
                    "construct",
                    vec![
                        Scheme::arr_judgment_lit(
                            "field",
                            vec![Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            )],
                            Value::from("param"),
                        ),
                        Scheme::arr_judgment_lit(
                            "field",
                            vec![Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.string"),
                            )],
                            Value::from("body"),
                        ),
                    ],
                    Value::from("type.function"),
                )],
                Value::from("param"),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 6. synth_last_child synthesizes the last child of a block
// ---------------------------------------------------------------------------

#[test]
fn synth_last_child_synthesizes_last_child() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // block rule: synth_last_child
    lang.types
        .merge(TypeRuleDef::new(
            "type.block",
            "block",
            Scheme::arr_judgment("synth_last_child", vec![]),
        ))
        .unwrap();
    // AST: [value(1), value(2), value(42), block{children:[0,1,2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("block").with_payload(json!({"children": [0, 1, 2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 7. synth_last_child on an empty block returns type.unit
// ---------------------------------------------------------------------------

#[test]
fn synth_last_child_empty_block_returns_unit() {
    let engine = engine();
    let mut lang = Language::new();
    // block rule: synth_last_child (empty block → type.unit)
    lang.types
        .merge(TypeRuleDef::new(
            "type.block",
            "block",
            Scheme::arr_judgment("synth_last_child", vec![]),
        ))
        .unwrap();
    // AST: [block{children:[]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("block").with_payload(json!({"children": []})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_unit());
}

// ---------------------------------------------------------------------------
// 8. call rule returns the function's body type
// ---------------------------------------------------------------------------

#[test]
fn call_rule_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // function rule: synth_child(0) >>> bind("paramtype")
    //   >>> synth_child(1) >>> bind("bodytype")
    //   >>> construct("type.function", [field("param", ...), field("body", ...)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("paramtype"),
                                        )],
                                        Value::from("param"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("bodytype"),
                                        )],
                                        Value::from("body"),
                                    ),
                                ],
                                Value::from("type.function"),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // call rule: synth_child(0) >>> bind("funtype")
    //   >>> check_child(1, [type_field("param", [result("funtype")])])
    //   >>> type_field("body", [result("funtype")])
    lang.types
        .merge(TypeRuleDef::new(
            "type.call",
            "call",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("funtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "type_field",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("funtype"),
                                )],
                                Value::from("param"),
                            )],
                            Value::from(1u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("funtype"),
                            )],
                            Value::from("body"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), function{children:[0,1]},
    //       value(42), call{children:[2,3]}]
    // The function node (idx 2) has children [value(42), value("hello")]
    //   → type.function{param:type.number, body:type.string}
    // The call node (idx 4) has children [function_node(2), value(42)(3)]
    //   → synths callee → type.function{...}
    //   → checks arg (value 42) against param (type.number) → OK
    //   → returns body (type.string)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("call").with_payload(json!({"children": [2, 3]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 9. check rule for type-annot: checks child 0 against expected type
// ---------------------------------------------------------------------------

#[test]
fn check_rule_type_annot() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // type-annot rule: synth = synth_child(0), check = check_child(0)
    // The check rule checks child 0 (body) against the expected type
    // (incoming).
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.type-annot",
                "type-annot",
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
            )
            .with_check(Scheme::arr_judgment_lit(
                "check_child",
                vec![],
                Value::from(0u64),
            )),
        )
        .unwrap();
    // wrapper rule: synth = check_child(0, [construct(type.number)])
    //   >>> construct(type.string)
    // The wrapper checks child 0 (type-annot) against type.number.
    // type-annot's check rule: check_child(0) → checks child 0 (value 42)
    //   against incoming (type.number) → value has no check rule →
    //   synth-then-unify: synth value → type.number, unify with type.number
    //   → OK. Then wrapper returns type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), type-annot{children:[0]}, wrapper{children:[1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("type-annot").with_payload(json!({"type": "x", "children": [0]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// ADR-0054 directed checking: check_json_value, check_last_child, and
// check rules for value/block/if/ternary nodes.
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. check_json_value: value(42) checked against type.number → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_json_value_matches_number() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth = construct(type.number) (dummy), check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // wrapper rule: synth = check_child(0, [construct(type.number)])
    //   >>> construct(type.string)
    // The wrapper checks child 0 (value 42) against type.number.
    // value's check rule: check_json_value with incoming=type.number →
    //   payload 42 is a number → succeeds. Then wrapper returns type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 2. check_json_value: value(42) checked against type.string → fails
// ---------------------------------------------------------------------------

#[test]
fn check_json_value_mismatch() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.string
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.string"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 3. check_json_value: value("hello") checked against type.string → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_json_value_matches_string() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.string
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.string"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    // AST: [value("hello"), wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 4. check_value_rule_uses_check_json_value: full value rule (synth + check)
//    end-to-end — value(42) checked against type.number → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_value_rule_uses_check_json_value() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth = node_payload >>> json_type_construct,
    //             check = check_json_value  (matches types.yaml)
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.number >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 5. check_block_rule_checks_last_child: block with last child value(42)
//    checked against type.number → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_block_rule_checks_last_child() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth = node_payload >>> json_type_construct,
    //             check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // block rule: synth = synth_last_child, check = check_last_child
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.block",
                "block",
                Scheme::arr_judgment("synth_last_child", vec![]),
            )
            .with_check(Scheme::arr_judgment("check_last_child", vec![])),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.number >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(1), value(42), block{children:[0,1]}, wrapper{children:[2]}]
    // The block's last child is value(42). The wrapper checks the block
    // against type.number. The block's check rule (check_last_child) checks
    // the last child (value 42) against type.number. value's check rule
    // (check_json_value) verifies 42 is a number → succeeds.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("block").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 6. check_if_rule_checks_both_branches: if with then=value(42),
//    else=value("hello") checked against type.number → fails (else mismatch)
// ---------------------------------------------------------------------------

#[test]
fn check_if_rule_checks_both_branches() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth = node_payload >>> json_type_construct,
    //             check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // if rule: synth = (the canonical synth), check = directed check
    //   bind("expected") >>> check_child(0, [type.bool])
    //   >>> check_child(1, [result("expected")])
    //   >>> check_child(2, [result("expected")])
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.if",
                "if",
                // synth: synth_child(0) >>> bind("tcond")
                //   >>> unify(result("tcond"), construct(type.bool))
                //   >>> synth_child(1) >>> bind("tthen")
                //   >>> unify(result("tthen"), synth_child(2))
                //   >>> result("tthen")
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("tcond")),
                        Scheme::seq(
                            Scheme::arr_judgment(
                                "unify",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("tcond"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "construct",
                                        vec![],
                                        Value::from("type.bool"),
                                    ),
                                ],
                            ),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "synth_child",
                                    vec![],
                                    Value::from(1u64),
                                ),
                                Scheme::seq(
                                    Scheme::arr_judgment_lit(
                                        "bind",
                                        vec![],
                                        Value::from("tthen"),
                                    ),
                                    Scheme::seq(
                                        Scheme::arr_judgment(
                                            "unify",
                                            vec![
                                                Scheme::arr_judgment_lit(
                                                    "result",
                                                    vec![],
                                                    Value::from("tthen"),
                                                ),
                                                Scheme::arr_judgment_lit(
                                                    "synth_child",
                                                    vec![],
                                                    Value::from(2u64),
                                                ),
                                            ],
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("tthen"),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.bool"),
                        )],
                        Value::from(0u64),
                    ),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from(1u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from(2u64),
                        ),
                    ),
                ),
            )),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.number >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(true), value(42), value("hello"),
    //       if{children:[0,1,2]}, wrapper{children:[3]}]
    // The wrapper checks the if node against type.number.
    // if's check rule:
    //   bind("expected") → stores type.number
    //   check_child(0, [type.bool]) → checks value(true) against type.bool → OK
    //   check_child(1, [result("expected")]) → checks value(42) against
    //     type.number → check_json_value: 42 is a number → OK
    //   check_child(2, [result("expected")]) → checks value("hello") against
    //     type.number → check_json_value: "hello" is not a number → FAILS
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [3]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error (else branch), got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 10. occurs-check prevents circular metavariable solving
// ---------------------------------------------------------------------------

#[test]
fn occurs_check_prevents_circular_meta() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical");
    // Allocate a fresh meta (id 0).
    let id = ctx.fresh_meta();
    // Build a value that contains the meta itself: {"__meta__": 0} nested
    // inside an array. Unifying the meta with this value must fail.
    let circular = json!([meta_value(id)]);
    let err = engine.unify(&meta_value(id), &circular, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("occurs-check failed for meta 0")),
        "expected occurs-check error, got {:?}",
        err
    );
    // The meta must remain unsolved.
    assert!(ctx.lookup_meta(id).is_none());
}

// ---------------------------------------------------------------------------
// 11. occurs-check allows non-circular values
// ---------------------------------------------------------------------------

#[test]
fn occurs_check_allows_non_circular() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical");
    let id = ctx.fresh_meta();
    // A value that does NOT contain the meta → unify should succeed and
    // solve the meta.
    let val = type_number();
    engine.unify(&meta_value(id), &val, &mut ctx).unwrap();
    assert_eq!(ctx.lookup_meta(id), Some(&type_number()));
}

// ---------------------------------------------------------------------------
// 12. template rule returns type.string
// ---------------------------------------------------------------------------

#[test]
fn template_rule_returns_type_string() {
    let engine = engine();
    let mut lang = Language::new();
    // template rule: construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.template",
            "template",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("template").with_payload(json!({"children": []})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 13. assign rule returns type.unit
// ---------------------------------------------------------------------------

#[test]
fn assign_rule_returns_type_unit() {
    let engine = engine();
    let mut lang = Language::new();
    // assign rule: construct(type.unit)
    lang.types
        .merge(TypeRuleDef::new(
            "type.assign",
            "assign",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.unit")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("assign").with_payload(json!({"children": []})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_unit());
}

// ---------------------------------------------------------------------------
// 14. loop rule returns type.unit
// ---------------------------------------------------------------------------

#[test]
fn loop_rule_returns_type_unit() {
    let engine = engine();
    let mut lang = Language::new();
    // loop rule: construct(type.unit)
    lang.types
        .merge(TypeRuleDef::new(
            "type.loop",
            "loop",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.unit")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("loop").with_payload(json!({"children": []})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_unit());
}

// ---------------------------------------------------------------------------
// 15. pipe rule returns the function's body type
// ---------------------------------------------------------------------------

#[test]
fn pipe_rule_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // function rule: synth_child(0) >>> bind("paramtype")
    //   >>> synth_child(1) >>> bind("bodytype")
    //   >>> construct("type.function", [field("param", ...), field("body", ...)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("paramtype"),
                                        )],
                                        Value::from("param"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("bodytype"),
                                        )],
                                        Value::from("body"),
                                    ),
                                ],
                                Value::from("type.function"),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // pipe rule: synth_child(1) >>> bind("funtype")
    //   >>> check_child(0, [type_field("param", [result("funtype")])])
    //   >>> type_field("body", [result("funtype")])
    // (like `call` but with children 0 and 1 swapped)
    lang.types
        .merge(TypeRuleDef::new(
            "type.pipe",
            "pipe",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("funtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "type_field",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("funtype"),
                                )],
                                Value::from("param"),
                            )],
                            Value::from(0u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("funtype"),
                            )],
                            Value::from("body"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), function{children:[0,1]},
    //       value(42), pipe{children:[3,2]}]
    // The function node (idx 2) has children [value(42), value("hello")]
    //   → type.function{param:type.number, body:type.string}
    // The pipe node (idx 4) has children [value(42)(3), function_node(2)]
    //   → synths child 1 (function) → type.function{...}
    //   → checks child 0 (value 42) against param (type.number) → OK
    //   → returns body (type.string)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("pipe").with_payload(json!({"children": [3, 2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// synth_match_arms — multi-arm unification for `match` nodes
// ===========================================================================

/// The expected tagged type value for `type.unknown` in `handl.canonical`.
fn type_unknown() -> Value {
    json!({"__tag__":"type.unknown","__registry__":"handl.canonical"})
}

/// Build a `Language` with `value`, `arm`, and `match` type rules for
/// `synth_match_arms` tests. The `value` rule is `node_payload >>>
/// json_type_construct`; the `match` rule is `synth_match_arms`; the `arm`
/// rule is a passthrough to its body (child 1) so it can be synthed
/// independently if needed.
fn match_test_lang() -> Language {
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // match rule: synth_match_arms
    lang.types
        .merge(TypeRuleDef::new(
            "type.match",
            "match",
            Scheme::arr_judgment("synth_match_arms", vec![]),
        ))
        .unwrap();
    lang
}

// ---------------------------------------------------------------------------
// 1. synth_match_arms unifies matching arms (both type.number)
// ---------------------------------------------------------------------------

#[test]
fn synth_match_arms_unifies_matching_arms() {
    let engine = engine();
    let lang = match_test_lang();
    // AST: [value(42), lit(true), value(42), arm{children:[1,2]},
    //       lit(true), value(42), arm{children:[4,5]},
    //       value(0), match{children:[7,3,6]}]
    //   scrutinee = value(0) (idx 7)
    //   arm1 (idx 3): pattern=lit(true)(idx 1), body=value(42)(idx 2) → type.number
    //   arm2 (idx 6): pattern=lit(true)(idx 4), body=value(42)(idx 5) → type.number
    //   Both arms unify to type.number.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42))); // 0: scrutinee value
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 1: pattern1
    ast.push(AstNode::new("value").with_payload(json!(42))); // 2: body1
    ast.push(AstNode::new("arm").with_payload(json!({"children": [1, 2]}))); // 3: arm1
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 4: pattern2
    ast.push(AstNode::new("value").with_payload(json!(42))); // 5: body2
    ast.push(AstNode::new("arm").with_payload(json!({"children": [4, 5]}))); // 6: arm2
    ast.push(AstNode::new("value").with_payload(json!(0))); // 7: scrutinee
    ast.push(
        AstNode::new("match").with_payload(json!({"children": [7, 3, 6]})), // 8: match
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 2. synth_match_arms errors on mismatched arms (type.number vs type.string)
// ---------------------------------------------------------------------------

#[test]
fn synth_match_arms_errors_on_mismatched_arms() {
    let engine = engine();
    let lang = match_test_lang();
    // AST: arm1 body = value(42) → type.number; arm2 body = value("hello")
    // → type.string. Unification fails.
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 0: pattern1
    ast.push(AstNode::new("value").with_payload(json!(42))); // 1: body1 → type.number
    ast.push(AstNode::new("arm").with_payload(json!({"children": [0, 1]}))); // 2: arm1
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 3: pattern2
    ast.push(AstNode::new("value").with_payload(json!("hello"))); // 4: body2 → type.string
    ast.push(AstNode::new("arm").with_payload(json!({"children": [3, 4]}))); // 5: arm2
    ast.push(AstNode::new("value").with_payload(json!(0))); // 6: scrutinee
    ast.push(
        AstNode::new("match").with_payload(json!({"children": [6, 2, 5]})), // 7: match
    );
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 3. synth_match_arms with a single arm returns that arm's body type
// ---------------------------------------------------------------------------

#[test]
fn synth_match_arms_single_arm() {
    let engine = engine();
    let lang = match_test_lang();
    // AST: [lit(true), value(42), arm{children:[0,1]}, value(0),
    //       match{children:[3,2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 0: pattern
    ast.push(AstNode::new("value").with_payload(json!(42))); // 1: body → type.number
    ast.push(AstNode::new("arm").with_payload(json!({"children": [0, 1]}))); // 2: arm
    ast.push(AstNode::new("value").with_payload(json!(0))); // 3: scrutinee
    ast.push(
        AstNode::new("match").with_payload(json!({"children": [3, 2]})), // 4: match
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 4. synth_match_arms with no arms (just scrutinee) returns type.unknown
// ---------------------------------------------------------------------------

#[test]
fn synth_match_arms_no_arms_returns_unknown() {
    let engine = engine();
    let lang = match_test_lang();
    // AST: [value(0), match{children:[0]}]  — only scrutinee, no arms
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(0))); // 0: scrutinee
    ast.push(
        AstNode::new("match").with_payload(json!({"children": [0]})), // 1: match
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_unknown());
    assert!(result.diagnostics.is_empty());
}

// ===========================================================================
// Record type support for `field-access` and array element type support
// for `index` nodes (ADR-0054).
// ===========================================================================

/// Build a `type.array` value carrying the given element type.
fn type_array(element: Value) -> Value {
    json!({
        "__tag__": "type.array",
        "__registry__": "handl.canonical",
        "element": element
    })
}

/// Build a `type.record` value with the given fields map.
fn type_record(fields: serde_json::Map<String, Value>) -> Value {
    json!({
        "__tag__": "type.record",
        "__registry__": "handl.canonical",
        "fields": Value::Object(fields)
    })
}

// ---------------------------------------------------------------------------
// 1. record_field_type extracts a field's type from a type.record value
// ---------------------------------------------------------------------------

#[test]
fn record_field_type_extracts_field() {
    let engine = engine();
    let mut lang = Language::new();
    // Build a type.record value with fields {name: type.string, age: type.number}.
    // Rule: const(record_type) >>> bind("rt")
    //   >>> record_field_type(result("rt"), lit="name")
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    let record_ty = type_record(fields);
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], record_ty),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("rt")),
                    Scheme::arr_judgment_lit(
                        "record_field_type",
                        vec![Scheme::arr_judgment_lit(
                            "result",
                            vec![],
                            Value::from("rt"),
                        )],
                        Value::from("name"),
                    ),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 2. record_field_type errors on a missing field
// ---------------------------------------------------------------------------

#[test]
fn record_field_type_missing_field_errors() {
    let engine = engine();
    let mut lang = Language::new();
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    let record_ty = type_record(fields);
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], record_ty),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("rt")),
                    Scheme::arr_judgment_lit(
                        "record_field_type",
                        vec![Scheme::arr_judgment_lit(
                            "result",
                            vec![],
                            Value::from("rt"),
                        )],
                        Value::from("email"),
                    ),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("field 'email' not found")),
        "expected missing-field error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 3. field-access rule returns the field's type from a record target
// ---------------------------------------------------------------------------

#[test]
fn field_access_rule_returns_field_type() {
    let engine = engine();
    let mut lang = Language::new();
    // "record" node: always synths to a fixed type.record value.
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    let record_ty = type_record(fields);
    lang.types
        .merge(TypeRuleDef::new(
            "type.record",
            "record",
            Scheme::arr_native_lit("const", vec![], record_ty),
        ))
        .unwrap();
    // field-access rule: synth_child(0) >>> bind("record_type")
    //   >>> payload_field("field") >>> record_field_type(result("record_type"))
    lang.types
        .merge(TypeRuleDef::new(
            "type.field-access",
            "field-access",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("record_type")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("payload_field", vec![], Value::from("field")),
                        Scheme::arr_judgment(
                            "record_field_type",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("record_type"),
                            )],
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [record, field-access{field: "name", children: [0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("record"));
    ast.push(
        AstNode::new("field-access")
            .with_payload(json!({"field": "name", "children": [0]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 4. array_element_type extracts the element type from a type.array value
// ---------------------------------------------------------------------------

#[test]
fn array_element_type_extracts_element() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: const(type.array{element: type.number}) >>> array_element_type
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], type_array(type_number())),
                Scheme::arr_judgment("array_element_type", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 5. array rule with elements carries the element type
// ---------------------------------------------------------------------------

#[test]
fn array_rule_with_elements_carries_element_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // array rule: synth_first_child_or_unknown >>> bind("element_type")
    //   >>> construct("type.array", [field("element", result("element_type"))])
    lang.types
        .merge(TypeRuleDef::new(
            "type.array",
            "array",
            Scheme::seq(
                Scheme::arr_judgment("synth_first_child_or_unknown", vec![]),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("element_type")),
                    Scheme::arr_judgment_lit(
                        "construct",
                        vec![Scheme::arr_judgment_lit(
                            "field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("element_type"),
                            )],
                            Value::from("element"),
                        )],
                        Value::from("type.array"),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(1), value(2), array{children: [0, 1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_array(type_number()));
}

// ---------------------------------------------------------------------------
// 6. index rule returns the element type of the target array
// ---------------------------------------------------------------------------

#[test]
fn index_rule_returns_element_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // array rule: synth_first_child_or_unknown >>> bind("element_type")
    //   >>> construct("type.array", [field("element", result("element_type"))])
    lang.types
        .merge(TypeRuleDef::new(
            "type.array",
            "array",
            Scheme::seq(
                Scheme::arr_judgment("synth_first_child_or_unknown", vec![]),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("element_type")),
                    Scheme::arr_judgment_lit(
                        "construct",
                        vec![Scheme::arr_judgment_lit(
                            "field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("element_type"),
                            )],
                            Value::from("element"),
                        )],
                        Value::from("type.array"),
                    ),
                ),
            ),
        ))
        .unwrap();
    // index rule: synth_child(0) >>> array_element_type
    lang.types
        .merge(TypeRuleDef::new(
            "type.index",
            "index",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::arr_judgment("array_element_type", vec![]),
            ),
        ))
        .unwrap();
    // AST: [value(1), value(2), array{children: [0, 1]},
    //       value(0), index{children: [2, 3]}]
    // The array node (idx 2) has children [value(1), value(2)]
    //   → type.array{element: type.number}
    // The index node (idx 4) has children [array_node(2), value(0)(3)]
    //   → synths child 0 (array) → type.array{element: type.number}
    //   → array_element_type → type.number
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(0)));
    ast.push(AstNode::new("index").with_payload(json!({"children": [2, 3]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ===========================================================================
// ADR-0054 directed check rules for binary arithmetic/comparison/logic and
// unary ops. These mirror the check rules added to types.yaml.
// ===========================================================================

/// Merge the canonical `value` rule (synth = node_payload >>> json_type_construct,
/// check = check_json_value) into the given language.
fn merge_value_rule(lang: &mut Language) {
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
}

/// Merge a `wrapper` rule that checks child 0 against the given expected type
/// tag, then returns `type.string` as a sentinel. The root of the AST is the
/// wrapper; its child 0 is the node under test.
fn merge_wrapper_rule(lang: &mut Language, expected_tag: &str) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from(expected_tag),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
}

/// Build the canonical `binary.add` synth scheme (both children type.number,
/// result type.number). Shared by all binary arithmetic ops in these tests.
fn binary_arith_synth() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        Scheme::seq(
            Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    ],
                ),
                Scheme::seq(
                    Scheme::arr_judgment(
                        "unify",
                        vec![
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                        ],
                    ),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ),
            ),
        ),
    )
}

/// Build the directed check scheme for binary arithmetic ops: verify the
/// expected type is type.number, then check both operands against type.number.
fn binary_arith_check() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
        Scheme::seq(
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_judgment_lit("result", vec![], Value::from("expected")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ],
            ),
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(1u64),
                ),
            ),
        ),
    )
}

/// Build the canonical `binary.eq` synth scheme (unify both operand types,
/// result type.bool). Shared by all binary comparison ops in these tests.
fn binary_compare_synth() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        Scheme::seq(
            Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
            ),
        ),
    )
}

/// Build the directed check scheme for binary comparison ops: verify the
/// expected type is type.bool, then synth child 0, bind its type, and check
/// child 1 against the same type.
fn binary_compare_check() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
        Scheme::seq(
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_judgment_lit("result", vec![], Value::from("expected")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                ],
            ),
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("operand_type")),
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "result",
                            vec![],
                            Value::from("operand_type"),
                        )],
                        Value::from(1u64),
                    ),
                ),
            ),
        ),
    )
}

/// Build the canonical `binary.and` synth scheme (both children type.bool,
/// result type.bool). Shared by all binary logic ops in these tests.
fn binary_logic_synth() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        Scheme::seq(
            Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                    ],
                ),
                Scheme::seq(
                    Scheme::arr_judgment(
                        "unify",
                        vec![
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                        ],
                    ),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                ),
            ),
        ),
    )
}

/// Build the directed check scheme for binary logic ops: verify the expected
/// type is type.bool, then check both operands against type.bool.
fn binary_logic_check() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
        Scheme::seq(
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_judgment_lit("result", vec![], Value::from("expected")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                ],
            ),
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.bool"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.bool"),
                    )],
                    Value::from(1u64),
                ),
            ),
        ),
    )
}

/// Build the canonical `unary.neg` synth scheme (child type.number, result
/// type.number). For `unary.not` pass `type.bool` instead.
fn unary_synth(result_tag: &str) -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        Scheme::seq(
            Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from(result_tag)),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from(result_tag)),
            ),
        ),
    )
}

/// Build the directed check scheme for unary ops: verify the expected type
/// matches `expected_tag`, then check the single operand against the same tag.
fn unary_check(expected_tag: &str) -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
        Scheme::seq(
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_judgment_lit("result", vec![], Value::from("expected")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from(expected_tag)),
                ],
            ),
            Scheme::arr_judgment_lit(
                "check_child",
                vec![Scheme::arr_judgment_lit(
                    "construct",
                    vec![],
                    Value::from(expected_tag),
                )],
                Value::from(0u64),
            ),
        ),
    )
}

// ---------------------------------------------------------------------------
// 1. check_binary_add_against_number_succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_binary_add_against_number_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.binary.add", "binary.add", binary_arith_synth())
                .with_check(binary_arith_check()),
        )
        .unwrap();
    merge_wrapper_rule(&mut lang, "type.number");
    // AST: [value(1), value(2), binary.add{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 2. check_binary_add_against_string_fails
// ---------------------------------------------------------------------------

#[test]
fn check_binary_add_against_string_fails() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.binary.add", "binary.add", binary_arith_synth())
                .with_check(binary_arith_check()),
        )
        .unwrap();
    // Wrapper checks binary.add against type.string — the check rule unifies
    // expected (type.string) with type.number → fails.
    merge_wrapper_rule(&mut lang, "type.string");
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure (add produces number, not string), got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 3. check_binary_eq_against_bool_succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_binary_eq_against_bool_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.binary.eq", "binary.eq", binary_compare_synth())
                .with_check(binary_compare_check()),
        )
        .unwrap();
    merge_wrapper_rule(&mut lang, "type.bool");
    // AST: [value(1), value(2), binary.eq{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 4. check_binary_eq_against_number_fails
// ---------------------------------------------------------------------------

#[test]
fn check_binary_eq_against_number_fails() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.binary.eq", "binary.eq", binary_compare_synth())
                .with_check(binary_compare_check()),
        )
        .unwrap();
    // Wrapper checks binary.eq against type.number — the check rule unifies
    // expected (type.number) with type.bool → fails.
    merge_wrapper_rule(&mut lang, "type.number");
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure (eq produces bool, not number), got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 5. check_binary_and_against_bool_succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_binary_and_against_bool_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.binary.and", "binary.and", binary_logic_synth())
                .with_check(binary_logic_check()),
        )
        .unwrap();
    merge_wrapper_rule(&mut lang, "type.bool");
    // AST: [value(true), value(false), binary.and{children:[0,1]},
    //       wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("value").with_payload(json!(false)));
    ast.push(AstNode::new("binary.and").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 6. check_unary_neg_against_number_succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_unary_neg_against_number_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.unary.neg", "unary.neg", unary_synth("type.number"))
                .with_check(unary_check("type.number")),
        )
        .unwrap();
    merge_wrapper_rule(&mut lang, "type.number");
    // AST: [value(1), unary.neg{children:[0]}, wrapper{children:[1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("unary.neg").with_payload(json!({"children": [0]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 7. check_unary_not_against_bool_succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_unary_not_against_bool_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(
            TypeRuleDef::new("type.unary.not", "unary.not", unary_synth("type.bool"))
                .with_check(unary_check("type.bool")),
        )
        .unwrap();
    merge_wrapper_rule(&mut lang, "type.bool");
    // AST: [value(true), unary.not{children:[0]}, wrapper{children:[1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("unary.not").with_payload(json!({"children": [0]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// flow_rule_returns_body_type — `flow` is like `pipe` (App reversed)
// ===========================================================================

#[test]
fn flow_rule_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // function rule: synth_child(0) >>> bind("paramtype")
    //   >>> synth_child(1) >>> bind("bodytype")
    //   >>> construct("type.function", [field("param", ...), field("body", ...)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("paramtype"),
                                        )],
                                        Value::from("param"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("bodytype"),
                                        )],
                                        Value::from("body"),
                                    ),
                                ],
                                Value::from("type.function"),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // flow rule: synth_child(1) >>> bind("funtype")
    //   >>> check_child(0, [type_field("param", [result("funtype")])])
    //   >>> type_field("body", [result("funtype")])
    // (like `pipe`: children [input(0), function(1)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.flow",
            "flow",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("funtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "type_field",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("funtype"),
                                )],
                                Value::from("param"),
                            )],
                            Value::from(0u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("funtype"),
                            )],
                            Value::from("body"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), function{children:[0,1]},
    //       value(42), flow{children:[3,2]}]
    // The function node (idx 2) has children [value(42), value("hello")]
    //   → type.function{param:type.number, body:type.string}
    // The flow node (idx 4) has children [value(42)(3), function_node(2)]
    //   → synths child 1 (function) → type.function{...}
    //   → checks child 0 (value 42) against param (type.number) → OK
    //   → returns body (type.string)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("flow").with_payload(json!({"children": [3, 2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// mood_int_rule_returns_body_type — `mood.int` is like `pipe`/`flow`
// ===========================================================================

#[test]
fn mood_int_rule_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // function rule (same as above)
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("paramtype"),
                                        )],
                                        Value::from("param"),
                                    ),
                                    Scheme::arr_judgment_lit(
                                        "field",
                                        vec![Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("bodytype"),
                                        )],
                                        Value::from("body"),
                                    ),
                                ],
                                Value::from("type.function"),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // mood.int rule: same as flow/pipe
    lang.types
        .merge(TypeRuleDef::new(
            "type.mood.int",
            "mood.int",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("funtype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "type_field",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("funtype"),
                                )],
                                Value::from("param"),
                            )],
                            Value::from(0u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("funtype"),
                            )],
                            Value::from("body"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), function{children:[0,1]},
    //       value(42), mood.int{children:[3,2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(
        AstNode::new("mood.int").with_payload(json!({"children": [3, 2]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// check_let_rule_checks_body — directed checking of `let` bodies
// ===========================================================================

#[test]
fn check_let_rule_checks_body() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth = node_payload >>> json_type_construct,
    //             check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // let rule: synth = (canonical), check = directed check
    //   bind("expected") >>> synth_child(0) >>> bind("vartype")
    //   >>> payload_field("name") >>> extend(result("vartype"))
    //   >>> check_child(1, [result("expected")])
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.let",
                "let",
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("vartype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "extend",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("vartype"),
                                    )],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            ),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("vartype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "extend",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("vartype"),
                                    )],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit(
                                    "check_child",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("expected"),
                                    )],
                                    Value::from(1u64),
                                ),
                            ),
                        ),
                    ),
                ),
            )),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.string >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.string"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), let{name:"x", children:[0,1]},
    //       wrapper{children:[2]}]
    // The wrapper checks the let node against type.string.
    // let's check rule:
    //   bind("expected") → stores type.string
    //   synth_child(0) → value(42) → type.number → bind("vartype")
    //   payload_field("name") → "x"
    //   extend(result("vartype")) → Γ += ("x", type.number)
    //   check_child(1, [result("expected")]) → checks value("hello") against
    //     type.string → check_json_value: "hello" is a string → OK
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
    );
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// check_let_rule_fails_on_body_mismatch
// ===========================================================================

#[test]
fn check_let_rule_fails_on_body_mismatch() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth + check
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // let rule: synth + check (same as above)
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.let",
                "let",
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("vartype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "extend",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("vartype"),
                                    )],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            ),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("vartype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("payload_field", vec![], Value::from("name")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "extend",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("vartype"),
                                    )],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit(
                                    "check_child",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("expected"),
                                    )],
                                    Value::from(1u64),
                                ),
                            ),
                        ),
                    ),
                ),
            )),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.number >>> construct(type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), let{name:"x", children:[0,1]},
    //       wrapper{children:[2]}]
    // The wrapper checks the let node against type.number.
    // let's check rule checks body value("hello") against type.number → FAILS
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
    );
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error (body), got {:?}",
        err
    );
}

// ===========================================================================
// check_function_rule_checks_both_children
// ===========================================================================

#[test]
fn check_function_rule_checks_both_children() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth + check
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // function rule: synth = (canonical), check = directed check
    //   bind("expected") >>> check_child(0, [type_field("param", [result("expected")])])
    //   >>> check_child(1, [type_field("body", [result("expected")])])
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.function",
                "function",
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            Scheme::seq(
                                Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("paramtype"),
                                            )],
                                            Value::from("param"),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("bodytype"),
                                            )],
                                            Value::from("body"),
                                        ),
                                    ],
                                    Value::from("type.function"),
                                ),
                            ),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from("param"),
                        )],
                        Value::from(0u64),
                    ),
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from("body"),
                        )],
                        Value::from(1u64),
                    ),
                ),
            )),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.function{number,string}
    //   >>> construct(type.string)
    let expected_fn = type_function(type_number(), type_string());
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit("const", vec![], expected_fn.clone())],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), function{children:[0,1]},
    //       wrapper{children:[2]}]
    // The wrapper checks the function node against
    //   type.function{param:type.number, body:type.string}.
    // function's check rule:
    //   bind("expected") → stores the type.function value
    //   check_child(0, [type_field("param", [result("expected")])])
    //     → checks value(42) against type.number → OK
    //   check_child(1, [type_field("body", [result("expected")])])
    //     → checks value("hello") against type.string → OK
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// check_function_rule_fails_on_param_mismatch
// ===========================================================================

#[test]
fn check_function_rule_fails_on_param_mismatch() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: synth + check
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // function rule: synth + check (same as above)
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.function",
                "function",
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("paramtype")),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            Scheme::seq(
                                Scheme::arr_judgment_lit("bind", vec![], Value::from("bodytype")),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("paramtype"),
                                            )],
                                            Value::from("param"),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("bodytype"),
                                            )],
                                            Value::from("body"),
                                        ),
                                    ],
                                    Value::from("type.function"),
                                ),
                            ),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from("param"),
                        )],
                        Value::from(0u64),
                    ),
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from("body"),
                        )],
                        Value::from(1u64),
                    ),
                ),
            )),
        )
        .unwrap();
    // wrapper rule: check child 0 against type.function{number,string}
    //   >>> construct(type.string)
    let expected_fn = type_function(type_number(), type_string());
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit("const", vec![], expected_fn.clone())],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value("hello"), value("hello"), function{children:[0,1]},
    //       wrapper{children:[2]}]
    // The wrapper checks the function node against
    //   type.function{param:type.number, body:type.string}.
    // function's check rule checks param value("hello") against type.number
    //   → FAILS (string is not a number)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error (param), got {:?}",
        err
    );
}

// ===========================================================================
// handle type rule, check_all_children judgment op, and array check rule
// (ADR-0054).
// ===========================================================================

/// Build a `Language` with a `value` rule (synth + check via
/// `check_json_value`) and an `array` rule (synth + check via
/// `check_all_children`). Used by the array-check tests below.
fn array_check_lang() -> Language {
    let mut lang = Language::new();
    // value rule: synth = node_payload >>> json_type_construct,
    //             check = check_json_value
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.value",
                "value",
                Scheme::seq(
                    Scheme::arr_judgment("node_payload", vec![]),
                    Scheme::arr_judgment("json_type_construct", vec![]),
                ),
            )
            .with_check(Scheme::arr_judgment("check_json_value", vec![])),
        )
        .unwrap();
    // array rule: synth = synth_first_child_or_unknown >>> bind("element_type")
    //   >>> construct("type.array", [field("element", result("element_type"))])
    //   check = bind("expected")
    //     >>> check_all_children(array_element_type(result("expected")))
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.array",
                "array",
                Scheme::seq(
                    Scheme::arr_judgment("synth_first_child_or_unknown", vec![]),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "bind",
                            vec![],
                            Value::from("element_type"),
                        ),
                        Scheme::arr_judgment_lit(
                            "construct",
                            vec![Scheme::arr_judgment_lit(
                                "field",
                                vec![Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("element_type"),
                                )],
                                Value::from("element"),
                            )],
                            Value::from("type.array"),
                        ),
                    ),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::arr_judgment(
                    "check_all_children",
                    vec![Scheme::arr_judgment(
                        "array_element_type",
                        vec![Scheme::arr_judgment_lit(
                            "result",
                            vec![],
                            Value::from("expected"),
                        )],
                    )],
                ),
            )),
        )
        .unwrap();
    lang
}

// ---------------------------------------------------------------------------
// 1. handle_rule_returns_body_type: handle with body value(42) → type.number
// ---------------------------------------------------------------------------

#[test]
fn handle_rule_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    // value rule: node_payload >>> json_type_construct
    lang.types
        .merge(TypeRuleDef::new(
            "type.value",
            "value",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("json_type_construct", vec![]),
            ),
        ))
        .unwrap();
    // handle rule: synth = synth_child(0)  (the body's type)
    lang.types
        .merge(TypeRuleDef::new(
            "type.handle",
            "handle",
            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        ))
        .unwrap();
    // AST: [value(42), block{children:[]}, handle{children:[0, 1]}]
    // The handle's child 0 is the body (value 42 → type.number); child 1 is
    // the block of handler clauses (ignored by the type rule).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("block").with_payload(json!({"children": []})));
    ast.push(AstNode::new("handle").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 2. check_all_children_checks_all_elements: array [1,2,3] checked against
//    type.array{element: type.number} → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_all_children_checks_all_elements() {
    let engine = engine();
    let lang = array_check_lang();
    // AST: [value(1), value(2), value(3), array{children:[0,1,2]},
    //       wrapper{children:[3]}]
    // The wrapper checks the array against type.array{element: type.number}.
    // The array's check rule extracts the element type (type.number) and
    // checks all children (values 1, 2, 3) against it → succeeds.
    let mut lang = lang;
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit(
                        "const",
                        vec![],
                        type_array(type_number()),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1, 2]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [3]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 3. check_all_children_fails_on_mismatch: array [1,2,3] checked against
//    type.array{element: type.string} → fails (elements are numbers)
// ---------------------------------------------------------------------------

#[test]
fn check_all_children_fails_on_mismatch() {
    let engine = engine();
    let mut lang = array_check_lang();
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit(
                        "const",
                        vec![],
                        type_array(type_string()),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(1), value(2), value(3), array{children:[0,1,2]},
    //       wrapper{children:[3]}]
    // The wrapper checks the array against type.array{element: type.string}.
    // The array's check rule extracts type.string and checks all children
    // (numbers) against it → fails on the first element.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1, 2]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [3]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 4. check_array_rule_against_matching_array_type: full end-to-end check of
//    an array of numbers against type.array{element: type.number} → succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_array_rule_against_matching_array_type() {
    let engine = engine();
    let mut lang = array_check_lang();
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit(
                        "const",
                        vec![],
                        type_array(type_number()),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.unit")),
            ),
        ))
        .unwrap();
    // AST: [value(1), value(2), value(3), array{children:[0,1,2]},
    //       wrapper{children:[3]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1, 2]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [3]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({"__tag__":"type.unit","__registry__":"handl.canonical"})
    );
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 5. check_array_rule_fails_on_element_mismatch: array [1, "hello"] checked
//    against type.array{element: type.number} → fails (second element is string)
// ---------------------------------------------------------------------------

#[test]
fn check_array_rule_fails_on_element_mismatch() {
    let engine = engine();
    let mut lang = array_check_lang();
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit(
                        "const",
                        vec![],
                        type_array(type_number()),
                    )],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [value(1), value("hello"), array{children:[0,1]},
    //       wrapper{children:[2]}]
    // The wrapper checks the array against type.array{element: type.number}.
    // The first element (1) checks fine, but the second ("hello") is a string
    // → fails.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("array").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check_json_value mismatch error (second element), got {:?}",
        err
    );
}

// ===========================================================================
// mood.cnd / mood.neg / mood.norm / mood.pot — modal arithmetic ops
// (all children type.number → type.number)
// ===========================================================================

/// Build the canonical mood.cnd synth scheme (3 children, all type.number,
/// result type.number).
fn mood_cnd_synth() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit(
            "check_child",
            vec![Scheme::arr_judgment_lit(
                "construct",
                vec![],
                Value::from("type.number"),
            )],
            Value::from(0u64),
        ),
        Scheme::seq(
            Scheme::arr_judgment_lit(
                "check_child",
                vec![Scheme::arr_judgment_lit(
                    "construct",
                    vec![],
                    Value::from("type.number"),
                )],
                Value::from(1u64),
            ),
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_judgment_lit(
                        "construct",
                        vec![],
                        Value::from("type.number"),
                    )],
                    Value::from(2u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ),
    )
}

/// Build the canonical mood.neg/norm/pot synth scheme (2 children, all
/// type.number, result type.number).
fn mood_binary_synth() -> Scheme {
    Scheme::seq(
        Scheme::arr_judgment_lit(
            "check_child",
            vec![Scheme::arr_judgment_lit(
                "construct",
                vec![],
                Value::from("type.number"),
            )],
            Value::from(0u64),
        ),
        Scheme::seq(
            Scheme::arr_judgment_lit(
                "check_child",
                vec![Scheme::arr_judgment_lit(
                    "construct",
                    vec![],
                    Value::from("type.number"),
                )],
                Value::from(1u64),
            ),
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ),
    )
}

// ---------------------------------------------------------------------------
// mood_cnd_rule_returns_type_number
// ---------------------------------------------------------------------------

#[test]
fn mood_cnd_rule_returns_type_number() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(TypeRuleDef::new(
            "type.mood.cnd",
            "mood.cnd",
            mood_cnd_synth(),
        ))
        .unwrap();
    // AST: [value(1), value(2), value(3), mood.cnd{children:[0,1,2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(
        AstNode::new("mood.cnd").with_payload(json!({"children": [0, 1, 2]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// mood_neg_rule_returns_type_number
// ---------------------------------------------------------------------------

#[test]
fn mood_neg_rule_returns_type_number() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(TypeRuleDef::new(
            "type.mood.neg",
            "mood.neg",
            mood_binary_synth(),
        ))
        .unwrap();
    // AST: [value(1), value(2), mood.neg{children:[0,1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(
        AstNode::new("mood.neg").with_payload(json!({"children": [0, 1]})),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// mood_cnd_rule_fails_on_non_number
// ---------------------------------------------------------------------------

#[test]
fn mood_cnd_rule_fails_on_non_number() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    lang.types
        .merge(TypeRuleDef::new(
            "type.mood.cnd",
            "mood.cnd",
            mood_cnd_synth(),
        ))
        .unwrap();
    // AST: [value(1), value("hello"), value(3), mood.cnd{children:[0,1,2]}]
    // Child 1 is a string → check_child against type.number fails.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(
        AstNode::new("mood.cnd").with_payload(json!({"children": [0, 1, 2]})),
    );
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected type mismatch error (string vs number), got {:?}",
        err
    );
}

// ===========================================================================
// object synth → type.record and object check against type.record
// ===========================================================================

/// Merge the canonical `object` rule (synth = synth_object_record,
/// check = check_object_record) into the given language.
fn merge_object_rule(lang: &mut Language) {
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.object",
                "object",
                Scheme::arr_judgment("synth_object_record", vec![]),
            )
            .with_check(Scheme::arr_judgment("check_object_record", vec![])),
        )
        .unwrap();
}

// ---------------------------------------------------------------------------
// object_synth_returns_type_record
// ---------------------------------------------------------------------------

#[test]
fn object_synth_returns_type_record() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
    // AST: [object{entries: [{key:"name", value:"hello"},
    //                        {key:"age", value:42}]}]
    let mut ast = Ast::new();
    ast.push(
        AstNode::new("object").with_payload(json!({
            "entries": [
                {"key": "name", "value": "hello"},
                {"key": "age", "value": 42}
            ]
        })),
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    assert_eq!(result.typ, type_record(fields));
}

// ---------------------------------------------------------------------------
// object_check_against_matching_record
// ---------------------------------------------------------------------------

#[test]
fn object_check_against_matching_record() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
    // Wrapper rule: check child 0 against a fixed type.record, return
    // type.string as a sentinel.
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    let record_ty = type_record(fields);
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit("const", vec![], record_ty)],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [object{entries: [...]}, wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(
        AstNode::new("object").with_payload(json!({
            "entries": [
                {"key": "name", "value": "hello"},
                {"key": "age", "value": 42}
            ]
        })),
    );
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ===========================================================================
// ADR-0054 dependent-type closure machinery: Pi/Sigma construction,
// closure application, unification, and lambda checking.
// ===========================================================================

/// Merge a `var` rule that looks up the variable name from `incoming`
/// (node_payload >>> lookup).
fn merge_var_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.var",
            "var",
            Scheme::seq(
                Scheme::arr_judgment("node_payload", vec![]),
                Scheme::arr_judgment("lookup", vec![]),
            ),
        ))
        .unwrap();
}

/// Merge a `pi` rule: synth = construct_pi(synth_child(0), lit="x").
fn merge_pi_rule(lang: &mut Language) {
    lang.types
        .merge(
            TypeRuleDef::new(
                "type.pi",
                "pi",
                Scheme::arr_judgment_lit(
                    "construct_pi",
                    vec![Scheme::arr_judgment_lit(
                        "synth_child",
                        vec![],
                        Value::from(0u64),
                    )],
                    Value::from("x"),
                ),
            )
            .with_check(Scheme::seq(
                Scheme::arr_judgment_lit("bind", vec![], Value::from("expected")),
                Scheme::seq(
                    Scheme::arr_judgment_lit(
                        "check_child",
                        vec![Scheme::arr_judgment_lit(
                            "type_field",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("expected"),
                            )],
                            Value::from("param"),
                        )],
                        Value::from(0u64),
                    ),
                    Scheme::arr_judgment_lit(
                        "check_pi_body",
                        vec![Scheme::arr_judgment_lit(
                            "result",
                            vec![],
                            Value::from("expected"),
                        )],
                        Value::Null,
                    ),
                ),
            )),
        )
        .unwrap();
}

/// Merge a `sigma` rule: synth = construct_sigma(synth_child(0), lit="x").
fn merge_sigma_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.sigma",
            "sigma",
            Scheme::arr_judgment_lit(
                "construct_sigma",
                vec![Scheme::arr_judgment_lit(
                    "synth_child",
                    vec![],
                    Value::from(0u64),
                )],
                Value::from("x"),
            ),
        ))
        .unwrap();
}

/// Merge a `forall` rule: synth = construct_forall(synth_child(0), lit="x").
fn merge_forall_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.forall",
            "forall",
            Scheme::arr_judgment_lit(
                "construct_forall",
                vec![Scheme::arr_judgment_lit(
                    "synth_child",
                    vec![],
                    Value::from(0u64),
                )],
                Value::from("x"),
            ),
        ))
        .unwrap();
}

/// Merge a `vau` rule: synth = construct_vau_type(synth_child(0), lit="x").
fn merge_vau_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.vau",
            "vau",
            Scheme::arr_judgment_lit(
                "construct_vau_type",
                vec![Scheme::arr_judgment_lit(
                    "synth_child",
                    vec![],
                    Value::from(0u64),
                )],
                Value::from("x"),
            ),
        ))
        .unwrap();
}

// ---------------------------------------------------------------------------
// 1. construct_pi creates a type.pi with body_closure
// ---------------------------------------------------------------------------

#[test]
fn pi_synth_creates_closure_based_pi() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // AST: [value(42), value("hello"), pi{children:[0,1]}]
    // Child 0 (value 42) synths to type.number (the param type).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.pi with param=type.number and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.pi")
    );
    assert_eq!(
        result.typ.get("param"),
        Some(&type_number())
    );
    let body_closure = result.typ.get("body_closure").expect("has body_closure");
    assert_eq!(
        body_closure.get("var_name").and_then(|v| v.as_str()),
        Some("x")
    );
    assert_eq!(
        body_closure.get("body_child_idx").and_then(|v| v.as_u64()),
        Some(1u64)
    );
}

// ---------------------------------------------------------------------------
// 2. construct_sigma creates a type.sigma with body_closure
// ---------------------------------------------------------------------------

#[test]
fn sigma_synth_creates_closure_based_sigma() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_sigma_rule(&mut lang);
    // AST: [value(42), value("hello"), sigma{children:[0,1]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("sigma").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.sigma")
    );
    assert_eq!(
        result.typ.get("param"),
        Some(&type_number())
    );
    let body_closure = result.typ.get("body_closure").expect("has body_closure");
    assert_eq!(
        body_closure.get("var_name").and_then(|v| v.as_str()),
        Some("x")
    );
    assert_eq!(
        body_closure.get("body_child_idx").and_then(|v| v.as_u64()),
        Some(1u64)
    );
}

// ---------------------------------------------------------------------------
// 3. pi_apply returns the body type for a specific argument
// ---------------------------------------------------------------------------

#[test]
fn pi_apply_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper rule: synth child 0 (the pi node) >>> bind("pitype")
    //   >>> pi_apply(result("pitype"), construct(type.number))
    // The pi_apply will bind "x" → type.number in Γ, synth the body child
    // (a var node "x"), which looks up "x" → type.number, and return
    // type.number.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pitype")),
                    Scheme::arr_judgment_lit(
                        "pi_apply",
                        vec![
                            Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("pitype"),
                            ),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
                        ],
                        Value::Null,
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 4. pi_apply with non-dependent body (body doesn't reference the variable)
// ---------------------------------------------------------------------------

#[test]
fn pi_apply_non_dependent_body() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper rule: synth child 0 (the pi node) >>> bind("pitype")
    //   >>> pi_apply(result("pitype"), construct(type.number))
    // The body child is a value("hello") which synths to type.string
    // regardless of the bound variable.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pitype")),
                    Scheme::arr_judgment_lit(
                        "pi_apply",
                        vec![
                            Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("pitype"),
                            ),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
                        ],
                        Value::Null,
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), pi{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 5. unify_pi succeeds on matching Pi types
// ---------------------------------------------------------------------------

#[test]
fn unify_pi_succeeds_on_matching_types() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper rule: synth child 0 (pi_a) >>> bind("pa")
    //   >>> synth child 1 (pi_b) >>> bind("pb")
    //   >>> unify_pi(result("pa"), result("pb"))
    //   >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pa")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("pb")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "unify_pi",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pa"),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pb"),
                                        ),
                                    ],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.string"),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi_a{children:[0,1]},
    //       value(42), var("x"), pi_b{children:[3,4]},
    //       wrapper{children:[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [3, 4]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 6. unify_pi fails on mismatched param types
// ---------------------------------------------------------------------------

#[test]
fn unify_pi_fails_on_mismatched_params() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pa")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("pb")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "unify_pi",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pa"),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pb"),
                                        ),
                                    ],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.string"),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi_a{children:[0,1]},
    //       value("hello"), var("x"), pi_b{children:[3,4]},
    //       wrapper{children:[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [3, 4]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2, 5]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unify failure (mismatched params), got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 7. unify (generic) skips body_closure for type.pi
// ---------------------------------------------------------------------------

#[test]
fn unify_skips_body_closure_for_pi() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper rule: synth child 0 (pi_a) >>> bind("pa")
    //   >>> synth child 1 (pi_b) >>> bind("pb")
    //   >>> unify(result("pa"), result("pb"))
    //   >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pa")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("pb")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "unify",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pa"),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "result",
                                            vec![],
                                            Value::from("pb"),
                                        ),
                                    ],
                                    Value::Null,
                                ),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.string"),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), pi_a{children:[0,1]},
    //       value(42), value("world"), pi_b{children:[3,4]},
    //       wrapper{children:[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("world")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [3, 4]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 8. context mark/restore works (closure application is scoped)
// ---------------------------------------------------------------------------

#[test]
fn context_mark_restore_works() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper: synth_child(0) >>> bind("pitype")
    //   >>> pi_apply(result("pitype"), construct(type.number))
    //   >>> bind("bodyresult")
    //   >>> lookup("x")   // should FAIL — "x" was scoped to the closure
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pitype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "pi_apply",
                            vec![
                                Scheme::arr_judgment_lit(
                                    "result",
                                    vec![],
                                    Value::from("pitype"),
                                ),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.number"),
                                ),
                            ],
                            Value::Null,
                        ),
                        Scheme::seq(
                            Scheme::arr_judgment_lit(
                                "bind",
                                vec![],
                                Value::from("bodyresult"),
                            ),
                            Scheme::arr_judgment_lit("lookup", vec![], Value::from("x")),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("unbound variable 'x'")),
        "expected unbound variable 'x' (context was restored), got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 9. check_pi_body: check a function against a Pi type
// ---------------------------------------------------------------------------

#[test]
fn check_pi_body_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    // Wrapper: synth child 0 (pi_a) >>> bind("pitype")
    //   >>> check_child(1, result("pitype"))  // check pi_b against pi_a
    //   >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pitype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("pitype"),
                            )],
                            Value::from(1u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.string"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi_a{children:[0,1]},
    //       value(42), var("x"), pi_b{children:[3,4]},
    //       wrapper{children:[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [3, 4]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 10. check_pi_body fails on mismatched body type
// ---------------------------------------------------------------------------

#[test]
fn check_pi_body_fails_on_mismatched_body() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_pi_rule(&mut lang);
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("pitype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit(
                            "check_child",
                            vec![Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("pitype"),
                            )],
                            Value::from(1u64),
                        ),
                        Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.string"),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), pi_a{children:[0,1]},
    //       value(42), value("hello"), pi_b{children:[3,4]},
    //       wrapper{children:[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [3, 4]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2, 5]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    // The value node's check rule (check_json_value) produces a "does not
    // match expected type" error when the payload doesn't match the expected
    // type tag.
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("does not match expected type")),
        "expected check failure (body type mismatch), got {:?}",
        err
    );
}

// ===========================================================================
// fanout_rule_returns_function_type — `fanout` is a function combinator;
// its result is type.function{param: type.unknown, body: type.unknown}
// ===========================================================================

#[test]
fn fanout_rule_returns_function_type() {
    let engine = engine();
    let mut lang = Language::new();
    // fanout rule: construct(type.function, [field("param", construct(type.unknown)),
    //                                   field("body",  construct(type.unknown))])
    lang.types
        .merge(TypeRuleDef::new(
            "type.fanout",
            "fanout",
            Scheme::arr_judgment_lit(
                "construct",
                vec![
                    Scheme::arr_judgment_lit(
                        "field",
                        vec![Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.unknown"),
                        )],
                        Value::from("param"),
                    ),
                    Scheme::arr_judgment_lit(
                        "field",
                        vec![Scheme::arr_judgment_lit(
                            "construct",
                            vec![],
                            Value::from("type.unknown"),
                        )],
                        Value::from("body"),
                    ),
                ],
                Value::from("type.function"),
            ),
        ))
        .unwrap();
    // AST: [value(42), value("hello"), fanout{children:[0,1]}]
    // The fanout node is the root (last in arena). Its children are functions
    // but the type rule doesn't inspect them — it just returns a function type.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("fanout").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        type_function(type_unknown(), type_unknown())
    );
}

// ===========================================================================
// Forall (parametric polymorphism) tests — ADR-0054
// ===========================================================================

// ---------------------------------------------------------------------------
// forall_synth_creates_closure_based_forall — synth a `forall` node → returns
// type.forall with body_closure
// ---------------------------------------------------------------------------

#[test]
fn forall_synth_creates_closure_based_forall() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_forall_rule(&mut lang);
    // AST: [value(42), value("hello"), forall{children:[0,1]}]
    // Child 0 (value 42) synths to type.number (the kind of the type param).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("forall").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.forall with kind=type.number and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.forall")
    );
    assert_eq!(
        result.typ.get("kind"),
        Some(&type_number())
    );
    let body_closure = result.typ.get("body_closure").expect("has body_closure");
    assert_eq!(
        body_closure.get("var_name").and_then(|v| v.as_str()),
        Some("x")
    );
    assert_eq!(
        body_closure.get("body_child_idx").and_then(|v| v.as_u64()),
        Some(1u64)
    );
}

// ---------------------------------------------------------------------------
// forall_apply_returns_body_type — apply a forall closure with a type
// argument → returns the body type
// ---------------------------------------------------------------------------

#[test]
fn forall_apply_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_forall_rule(&mut lang);
    // Wrapper rule: synth child 0 (the forall node) >>> bind("foralltype")
    //   >>> forall_apply(result("foralltype"), construct(type.number))
    // The forall_apply will bind "x" → type.number in Γ, synth the body child
    // (a var node "x"), which looks up "x" → type.number, and return
    // type.number.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("foralltype")),
                    Scheme::arr_judgment_lit(
                        "forall_apply",
                        vec![
                            Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("foralltype"),
                            ),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
                        ],
                        Value::Null,
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), forall{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("forall").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ===========================================================================
// Vau (fexpr) type tests — ADR-0054
// ===========================================================================

// ---------------------------------------------------------------------------
// vau_synth_creates_closure_based_vau — synth a `vau` node → returns
// type.vau with body_closure
// ---------------------------------------------------------------------------

#[test]
fn vau_synth_creates_closure_based_vau() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_vau_rule(&mut lang);
    // AST: [value(42), value("hello"), vau{children:[0,1]}]
    // Child 0 (value 42) synths to type.number (the param type).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("vau").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.vau with param=type.number and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.vau")
    );
    assert_eq!(
        result.typ.get("param"),
        Some(&type_number())
    );
    let body_closure = result.typ.get("body_closure").expect("has body_closure");
    assert_eq!(
        body_closure.get("var_name").and_then(|v| v.as_str()),
        Some("x")
    );
    assert_eq!(
        body_closure.get("body_child_idx").and_then(|v| v.as_u64()),
        Some(1u64)
    );
}

// ---------------------------------------------------------------------------
// vau_apply_returns_body_type — apply a vau closure with an argument →
// returns the body type
// ---------------------------------------------------------------------------

#[test]
fn vau_apply_returns_body_type() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_var_rule(&mut lang);
    merge_vau_rule(&mut lang);
    // Wrapper rule: synth child 0 (the vau node) >>> bind("vautype")
    //   >>> vau_apply(result("vautype"), construct(type.number))
    // The vau_apply will bind "x" → type.number in Γ, synth the body child
    // (a var node "x"), which looks up "x" → type.number, and return
    // type.number.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("vautype")),
                    Scheme::arr_judgment_lit(
                        "vau_apply",
                        vec![
                            Scheme::arr_judgment_lit(
                                "result",
                                vec![],
                                Value::from("vautype"),
                            ),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
                        ],
                        Value::Null,
                    ),
                ),
            ),
        ))
        .unwrap();
    // AST: [value(42), var("x"), vau{children:[0,1]}, wrapper{children:[2]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("vau").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ===========================================================================
// ADR-0054 universe level support
// ===========================================================================

/// The expected tagged type value for `type.universe` with a given level.
fn type_universe(level: u64) -> Value {
    json!({
        "__tag__": "type.universe",
        "__registry__": "handl.canonical",
        "level": level
    })
}

// ---------------------------------------------------------------------------
// construct_universe(0) → {__tag__: "type.universe", level: 0}
// ---------------------------------------------------------------------------

#[test]
fn construct_universe_creates_type_universe() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(lit=0)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_universe(0));
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// construct_universe(3) → {__tag__: "type.universe", level: 3}
// ---------------------------------------------------------------------------

#[test]
fn construct_universe_with_level() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(lit=3)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(3u64)),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_universe(3));
}

// ---------------------------------------------------------------------------
// construct_universe via arg sub-scheme (not lit)
// ---------------------------------------------------------------------------

#[test]
fn construct_universe_with_arg() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(args=[const(2)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "construct_universe",
                vec![Scheme::arr_native_lit("const", vec![], Value::from(2u64))],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_universe(2));
}

// ---------------------------------------------------------------------------
// unify Type_0 with Type_0 → succeeds
// ---------------------------------------------------------------------------

#[test]
fn unify_same_level_universes_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(0) >>> unify(lit=Type_0) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("unify", vec![], type_universe(0)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// unify Type_0 with Type_1 → fails
// ---------------------------------------------------------------------------

#[test]
fn unify_different_level_universes_fails() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(0) >>> unify(lit=Type_1) → fails
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
                Scheme::arr_judgment_lit("unify", vec![], type_universe(1)),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify Type_0 with Type_1")),
        "expected universe level mismatch, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// unify Type_1 with Type_1 (same non-zero level) → succeeds
// ---------------------------------------------------------------------------

#[test]
fn unify_same_nonzero_level_universes_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(2) >>> unify(lit=Type_2) >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(2u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("unify", vec![], type_universe(2)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// universe_level(construct_universe(2)) → 2
// ---------------------------------------------------------------------------

#[test]
fn universe_level_extracts_from_universe() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_universe(2) >>> universe_level(args=[id])
    // The id passes incoming (the Type_2 value) through to universe_level.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(2u64)),
                Scheme::arr_judgment(
                    "universe_level",
                    vec![Scheme::arr_native("id", vec![])],
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(2u64));
}

// ---------------------------------------------------------------------------
// universe_level(type.number) → 0 (base types live in Type_0)
// ---------------------------------------------------------------------------

#[test]
fn universe_level_returns_zero_for_base_type() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> universe_level(args=[id])
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::arr_judgment(
                    "universe_level",
                    vec![Scheme::arr_native("id", vec![])],
                ),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(0u64));
}

// ---------------------------------------------------------------------------
// universe_max(1, 3) → 3
// ---------------------------------------------------------------------------

#[test]
fn universe_max_computes_max() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: universe_max(args=[const(1), const(3)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "universe_max",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(1u64)),
                    Scheme::arr_native_lit("const", vec![], Value::from(3u64)),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(3u64));
}

// ---------------------------------------------------------------------------
// universe_max(5, 2) → 5 (order independence)
// ---------------------------------------------------------------------------

#[test]
fn universe_max_computes_max_reversed() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: universe_max(args=[const(5), const(2)])
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "universe_max",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(5u64)),
                    Scheme::arr_native_lit("const", vec![], Value::from(2u64)),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(5u64));
}

// ---------------------------------------------------------------------------
// universe_max with universe_level: max(level(Type_2), level(Type_4)) → 4
// ---------------------------------------------------------------------------

#[test]
fn universe_max_with_universe_levels() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: universe_max(
    //   universe_level(construct_universe(2)),
    //   universe_level(construct_universe(4)),
    // )
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "universe_max",
                vec![
                    Scheme::arr_judgment(
                        "universe_level",
                        vec![Scheme::arr_judgment_lit(
                            "construct_universe",
                            vec![],
                            Value::from(2u64),
                        )],
                    ),
                    Scheme::arr_judgment(
                        "universe_level",
                        vec![Scheme::arr_judgment_lit(
                            "construct_universe",
                            vec![],
                            Value::from(4u64),
                        )],
                    ),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::from(4u64));
}

// ===========================================================================
// Membership type tests (Is, Collection, Membership) — ADR-0054
// ===========================================================================

/// The expected tagged type value for `type.is_evidence` in `handl.canonical`.
fn type_is_evidence() -> Value {
    json!({"__tag__":"type.is_evidence","__registry__":"handl.canonical"})
}

// ---------------------------------------------------------------------------
// construct_membership_creates_type_membership — construct_membership(42,
// type.number) → {__tag__: "type.membership", value: 42, collection:
// type.number}
// ---------------------------------------------------------------------------

#[test]
fn construct_membership_creates_type_membership() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_membership(const(42), construct(type.number))
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "construct_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(42)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({
            "__tag__": "type.membership",
            "__registry__": "handl.canonical",
            "value": 42,
            "collection": type_number()
        })
    );
}

// ---------------------------------------------------------------------------
// check_membership_number_in_type_number — check_membership(42,
// type.number) → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_number_in_type_number() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(42)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// check_membership_string_in_type_number — check_membership("hello",
// type.number) → type.empty
// ---------------------------------------------------------------------------

#[test]
fn check_membership_string_in_type_number() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from("hello")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_empty());
}

// ---------------------------------------------------------------------------
// check_membership_number_in_type_string — check_membership(42,
// type.string) → type.empty
// ---------------------------------------------------------------------------

#[test]
fn check_membership_number_in_type_string() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(42)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_empty());
}

// ---------------------------------------------------------------------------
// check_membership_string_in_type_string — check_membership("hello",
// type.string) → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_string_in_type_string() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from("hello")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// check_membership_bool_in_type_bool — check_membership(true, type.bool)
// → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_bool_in_type_bool() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::from(true)),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// check_membership_null_in_type_null — check_membership(null, type.null)
// → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_null_in_type_null() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], Value::Null),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.null")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// check_membership_array_in_type_array — check_membership([1,2],
// type.array) → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_array_in_type_array() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], json!([1, 2])),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.array")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// check_membership_object_in_type_object — check_membership({},
// type.object) → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn check_membership_object_in_type_object() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_membership",
                vec![
                    Scheme::arr_native_lit("const", vec![], json!({})),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.object")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// eval_membership_reduces_to_evidence — eval_membership(
// type.membership{value: 42, collection: type.number}) → type.is_evidence
// ---------------------------------------------------------------------------

#[test]
fn eval_membership_reduces_to_evidence() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: eval_membership(construct_membership(const(42),
    // construct(type.number)))
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "eval_membership",
                vec![Scheme::arr_judgment(
                    "construct_membership",
                    vec![
                        Scheme::arr_native_lit("const", vec![], Value::from(42)),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    ],
                )],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_is_evidence());
}

// ---------------------------------------------------------------------------
// eval_membership_reduces_to_empty — eval_membership(
// type.membership{value: "hello", collection: type.number}) → type.empty
// ---------------------------------------------------------------------------

#[test]
fn eval_membership_reduces_to_empty() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "eval_membership",
                vec![Scheme::arr_judgment(
                    "construct_membership",
                    vec![
                        Scheme::arr_native_lit("const", vec![], Value::from("hello")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    ],
                )],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_empty());
}

// ---------------------------------------------------------------------------
// construct_collection_creates_type_collection — construct_collection(
// "type.number") → {__tag__: "type.collection", name: "type.number"}
// ---------------------------------------------------------------------------

#[test]
fn construct_collection_creates_type_collection() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment_lit("construct_collection", vec![], Value::from("type.number")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        json!({
            "__tag__": "type.collection",
            "__registry__": "handl.canonical",
            "name": "type.number"
        })
    );
}

// ===========================================================================
// ADR-0054 equality type support (Eq, Refl, Empty)
// ===========================================================================

/// The expected tagged type value for `type.eq` with given left/right fields.
fn type_eq(left: Value, right: Value) -> Value {
    json!({
        "__tag__": "type.eq",
        "__registry__": "handl.canonical",
        "left": left,
        "right": right
    })
}

/// The expected tagged type value for `type.empty` (no fields).
fn type_empty() -> Value {
    json!({"__tag__":"type.empty","__registry__":"handl.canonical"})
}

/// The expected tagged type value for `type.refl` (no fields).
fn type_refl() -> Value {
    json!({"__tag__":"type.refl","__registry__":"handl.canonical"})
}

// ---------------------------------------------------------------------------
// construct_eq_creates_type_eq — construct_eq(type.number, type.number)
// ---------------------------------------------------------------------------

#[test]
fn construct_eq_creates_type_eq() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct_eq(construct(type.number), construct(type.number))
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "construct_eq",
                vec![
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_eq(type_number(), type_number()));
}

// ---------------------------------------------------------------------------
// construct_empty_creates_type_empty — construct_empty()
// ---------------------------------------------------------------------------

#[test]
fn construct_empty_creates_type_empty() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment("construct_empty", vec![]),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_empty());
}

// ---------------------------------------------------------------------------
// check_refl_succeeds_when_sides_equal — check_refl against
// type.eq{left: 42, right: 42} succeeds
// ---------------------------------------------------------------------------

#[test]
fn check_refl_succeeds_when_sides_equal() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: check_refl(const(type.eq{left: 42, right: 42})) >>> construct(type.number)
    let eq_type = type_eq(Value::from(42), Value::from(42));
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "check_refl",
                    vec![Scheme::arr_native_lit("const", vec![], eq_type)],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// check_refl_fails_when_sides_differ — check_refl against
// type.eq{left: 42, right: "hello"} fails
// ---------------------------------------------------------------------------

#[test]
fn check_refl_fails_when_sides_differ() {
    let engine = engine();
    let mut lang = Language::new();
    let eq_type = type_eq(Value::from(42), Value::from("hello"));
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "check_refl",
                vec![Scheme::arr_native_lit("const", vec![], eq_type)],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("check_refl: cannot prove")),
        "expected check_refl failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// eq_reduce_returns_refl_when_equal — eq_reduce(type.eq{left: 42, right: 42})
// → type.refl
// ---------------------------------------------------------------------------

#[test]
fn eq_reduce_returns_refl_when_equal() {
    let engine = engine();
    let mut lang = Language::new();
    let eq_type = type_eq(Value::from(42), Value::from(42));
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "eq_reduce",
                vec![Scheme::arr_native_lit("const", vec![], eq_type)],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_refl());
}

// ---------------------------------------------------------------------------
// eq_reduce_returns_empty_when_not_equal —
// eq_reduce(type.eq{left: 42, right: "hello"}) → type.empty
// ---------------------------------------------------------------------------

#[test]
fn eq_reduce_returns_empty_when_not_equal() {
    let engine = engine();
    let mut lang = Language::new();
    let eq_type = type_eq(Value::from(42), Value::from("hello"));
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "eq_reduce",
                vec![Scheme::arr_native_lit("const", vec![], eq_type)],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_empty());
}

// ---------------------------------------------------------------------------
// unify_eq_types_unifies_both_sides — unify two identical type.eq values
// succeeds
// ---------------------------------------------------------------------------

#[test]
fn unify_eq_types_unifies_both_sides() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: unify(eq_a, eq_b) >>> construct(type.number)
    // Both eq values have left=type.number, right=type.string.
    let eq_a = type_eq(type_number(), type_string());
    let eq_b = type_eq(type_number(), type_string());
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "unify",
                    vec![
                        Scheme::arr_native_lit("const", vec![], eq_a),
                        Scheme::arr_native_lit("const", vec![], eq_b),
                    ],
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// unify_eq_types_fails_on_mismatch — unify type.eq values whose right sides
// differ fails
// ---------------------------------------------------------------------------

#[test]
fn unify_eq_types_fails_on_mismatch() {
    let engine = engine();
    let mut lang = Language::new();
    // eq_a: left=type.number, right=type.string
    // eq_b: left=type.number, right=type.number  (right sides differ)
    let eq_a = type_eq(type_number(), type_string());
    let eq_b = type_eq(type_number(), type_number());
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "unify",
                vec![
                    Scheme::arr_native_lit("const", vec![], eq_a),
                    Scheme::arr_native_lit("const", vec![], eq_b),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}
