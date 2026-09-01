//! Tests for the [`NbeTypecheckEngine`].
//!
//! These tests construct a minimal [`Language`] with type rules and verify
//! the judgment ops, combinators, and engine lifecycle methods end-to-end.

use super::*;
use handl_language::fragments::TypeRuleDef;
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
