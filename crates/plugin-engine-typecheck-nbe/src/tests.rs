//! Tests for the [`NbeTypecheckEngine`].
//!
//! These tests construct a minimal [`Language`] with type rules and verify
//! the judgment ops, combinators, and engine lifecycle methods end-to-end.

use super::*;
use handl_language::fragments::{AstShapeDef, ChildRoleSpec, EvalSchemeDef, TypeRuleDef};
use handl_language::{Ast, AstNode, Language, Scheme, Value};
use handl_platform::ModuleLookup;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;
use std::collections::HashMap;
use std::rc::Rc;

/// Build an [`NbeTypecheckEngine`] backed by a fresh [`TreeWalkEngine`].
fn engine() -> NbeTypecheckEngine {
    NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()))
}

/// The expected tagged type value for `type.number` in `handl.canonical`.
fn type_number() -> Value {
    json!({"__tag__":"type.number","__registry__":"handl.canonical"})
}

/// The expected tagged type value for `type.float` in `handl.canonical`.
fn type_float() -> Value {
    json!({"__tag__":"type.float","__registry__":"handl.canonical"})
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("empty ast")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("no type rule for kind 'lit'")),
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
// c'. node_types populated for root and child nodes
// ---------------------------------------------------------------------------

#[test]
fn node_types_populated_after_typecheck() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrap",
            "wrap",
            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 0
    ast.push(
        AstNode::new("wrap").with_payload(json!({"children": [0]})), // 1
    );
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert_eq!(result.node_types.get(&0), Some(&type_number()));
    assert_eq!(result.node_types.get(&1), Some(&type_number()));
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
    ast.push(AstNode::new("wrap").with_payload(json!({"children": [0]})));
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("unknown judgment op 'bogus_op'")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("not supported in type rules yet")),
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
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
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
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("foreign op")
            && message.contains("fs.read")
            && message.contains("pure-eval")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("effectful foreign ops")
            && message.contains("fs.read")
            && message.contains("pure-eval")),
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
    let engine = NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new())).with_pure(false);
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
        Err(LangError::Engine { ref message, .. }) => {
            assert!(
                !message.contains("effectful foreign ops"),
                "purity check should have been skipped (pure=false), but got: {}",
                message
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
    // Rule: const(42) >>> json_type_construct → type.int (integer literal)
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
    ast.push(AstNode::new("test").with_payload(json!({"name": "x", "children": [0, 1]})));
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

    // Number literal → type.int (integer literal)
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
    ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The body is var("x"), which looks up "x" in Γ → type.int.
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
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
                                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                                Scheme::seq(
                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("tthen")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    // Allocate a fresh meta (id 0).
    let id = ctx.fresh_meta();
    // Build a value that contains the meta itself: {"__meta__": 0} nested
    // inside an array. Unifying the meta with this value must fail.
    let circular = json!([meta_value(id)]);
    let err = engine
        .unify(&meta_value(id), &circular, &mut ctx)
        .unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("occurs-check failed for meta 0")),
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
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
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
    //   arm1 (idx 3): pattern=lit(true)(idx 1), body=value(42)(idx 2) → type.int
    //   arm2 (idx 6): pattern=lit(true)(idx 4), body=value(42)(idx 5) → type.int
    //   Both arms unify to type.int.
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
    ast.push(AstNode::new("value").with_payload(json!(42))); // 1: body → type.int
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("field 'email' not found")),
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
    ast.push(AstNode::new("field-access").with_payload(json!({"field": "name", "children": [0]})));
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
    //   → type.array{element: type.int}
    // The index node (idx 4) has children [array_node(2), value(0)(3)]
    //   → synths child 0 (array) → type.array{element: type.int}
    //   → array_element_type → type.int
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
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
    ast.push(AstNode::new("mood.int").with_payload(json!({"children": [3, 2]})));
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
    ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));
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
    ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
    // The handle's child 0 is the body (value 42 → type.int); child 1 is
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
    ast.push(AstNode::new("mood.cnd").with_payload(json!({"children": [0, 1, 2]})));
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
    ast.push(AstNode::new("mood.neg").with_payload(json!({"children": [0, 1]})));
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
    ast.push(AstNode::new("mood.cnd").with_payload(json!({"children": [0, 1, 2]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "hello"},
            {"key": "age", "value": 42}
        ]
    })));
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
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "hello"},
            {"key": "age", "value": 42}
        ]
    })));
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
    // Child 0 (value 42) synths to type.int (the param type).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("pi").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.pi with param=type.int and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.pi")
    );
    assert_eq!(result.typ.get("param"), Some(&type_number()));
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
    assert_eq!(result.typ.get("param"), Some(&type_number()));
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
                            Scheme::arr_judgment_lit("result", vec![], Value::from("pitype")),
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
                            Scheme::arr_judgment_lit("result", vec![], Value::from("pitype")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
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
                                Scheme::arr_judgment_lit("result", vec![], Value::from("pitype")),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.number"),
                                ),
                            ],
                            Value::Null,
                        ),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("bodyresult")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("unbound variable 'x'")),
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
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
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
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("does not match expected type")),
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
    assert_eq!(result.typ, type_function(type_unknown(), type_unknown()));
}

// ===========================================================================
// Arrow kind signatures — ADR-0074 follow-up #3
// ===========================================================================
//
// The categorical combinators `>>>` (flow), `***` (product), `&&&` (fanout),
// and `|||` (choice) now have precise arrow kind signatures:
//   >>>  f: A ~> B, g: B ~> C ⟹ f >>> g : A ~> C   (composition; v0.1 eval
//        is application, so the type rule matches application semantics)
//   ***  f: A ~> B, g: C ~> D ⟹ f *** g : (A, C) ~> (B, D)
//   &&&  f: A ~> B, g: A ~> C ⟹ f &&& g : A ~> (B, C)
//   |||  f: A ~> C, g: B ~> C ⟹ f ||| g : (A | B) ~> C
// where `A ~> B` is `type.function{param: A, body: B}`, `(A, C)` is a
// positional `type.array{elements: [A, C]}`, and `(A | B)` is
// `type.sum{variants: [A, B]}`.

/// A positional `type.array` with an `elements` field (a heterogeneous
/// tuple), used for product/fanout pair types.
fn type_array_elements(elements: Vec<Value>) -> Value {
    json!({
        "__tag__": "type.array",
        "__registry__": "handl.canonical",
        "elements": Value::Array(elements)
    })
}

/// A `type.sum` with a `variants` field, used for the choice input type.
fn type_sum(variants: Vec<Value>) -> Value {
    json!({
        "__tag__": "type.sum",
        "__registry__": "handl.canonical",
        "variants": Value::Array(variants)
    })
}

/// Merge the canonical `function` synth rule: synth child 0 (param) →
/// bind "paramtype", synth child 1 (body) → bind "bodytype", then construct
/// `type.function{param, body}`.
fn merge_function_rule(lang: &mut Language) {
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
}

/// Merge the canonical `fanout` (&&&) synth rule: synth both children,
/// extract param/body of each, unify the two params, construct
/// `type.function{param: A, body: type.array{elements: [B, C]}}`.
fn merge_fanout_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.fanout",
            "fanout",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("ftype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("gtype")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "type_field",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("ftype"),
                                    )],
                                    Value::from("param"),
                                ),
                                Scheme::seq(
                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("A1")),
                                    Scheme::seq(
                                        Scheme::arr_judgment_lit(
                                            "type_field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("ftype"),
                                            )],
                                            Value::from("body"),
                                        ),
                                        Scheme::seq(
                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("B")),
                                            Scheme::seq(
                                                Scheme::arr_judgment_lit(
                                                    "type_field",
                                                    vec![Scheme::arr_judgment_lit(
                                                        "result",
                                                        vec![],
                                                        Value::from("gtype"),
                                                    )],
                                                    Value::from("param"),
                                                ),
                                                Scheme::seq(
                                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("A2")),
                                                    Scheme::seq(
                                                        Scheme::arr_judgment_lit(
                                                            "type_field",
                                                            vec![Scheme::arr_judgment_lit(
                                                                "result",
                                                                vec![],
                                                                Value::from("gtype"),
                                                            )],
                                                            Value::from("body"),
                                                        ),
                                                        Scheme::seq(
                                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("C")),
                                                            Scheme::seq(
                                                                Scheme::arr_judgment(
                                                                    "unify",
                                                                    vec![
                                                                        Scheme::arr_judgment_lit(
                                                                            "result",
                                                                            vec![],
                                                                            Value::from("A1"),
                                                                        ),
                                                                        Scheme::arr_judgment_lit(
                                                                            "result",
                                                                            vec![],
                                                                            Value::from("A2"),
                                                                        ),
                                                                    ],
                                                                ),
                                                                Scheme::arr_judgment_lit(
                                                                    "construct",
                                                                    vec![
                                                                        Scheme::arr_judgment_lit(
                                                                            "field",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "result",
                                                                                vec![],
                                                                                Value::from("A1"),
                                                                            )],
                                                                            Value::from("param"),
                                                                        ),
                                                                        Scheme::arr_judgment_lit(
                                                                            "field",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "construct",
                                                                                vec![Scheme::arr_judgment_lit(
                                                                                    "field",
                                                                                    vec![Scheme::arr_judgment(
                                                                                        "make_array",
                                                                                        vec![
                                                                                            Scheme::arr_judgment_lit(
                                                                                                "result",
                                                                                                vec![],
                                                                                                Value::from("B"),
                                                                                            ),
                                                                                            Scheme::arr_judgment_lit(
                                                                                                "result",
                                                                                                vec![],
                                                                                                Value::from("C"),
                                                                                            ),
                                                                                        ],
                                                                                    )],
                                                                                    Value::from("elements"),
                                                                                )],
                                                                                Value::from("type.array"),
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
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
}

// ---------------------------------------------------------------------------
// fanout_rule_infers_arrow_kind_signature — `f &&& g` where
// f: A ~> B, g: A ~> C ⟹ f &&& g : A ~> (B, C)
// ---------------------------------------------------------------------------

#[test]
fn fanout_rule_infers_arrow_kind_signature() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_function_rule(&mut lang);
    merge_fanout_rule(&mut lang);
    // AST: [value(42), value("hi"), value(42), value(true),
    //       function{children:[0,1]}  // f: number ~> string  (idx 4)
    //       function{children:[2,3]}  // g: number ~> bool    (idx 5)
    //       fanout{children:[4,5]}]   // f &&& g : number ~> (string, bool)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("function").with_payload(json!({"children": [2, 3]})));
    ast.push(AstNode::new("fanout").with_payload(json!({"children": [4, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // Expected: type.function{param: type.int,
    //                         body: type.array{elements: [type.string, type.bool]}}
    assert_eq!(
        result.typ,
        type_function(
            type_number(),
            type_array_elements(vec![type_string(), type_bool()])
        )
    );
}

// ---------------------------------------------------------------------------
// fanout_rule_param_mismatch_errors — `f &&& g` requires both arrows to
// share the same input type; a mismatch unifies type.number with type.string
// and errors.
// ---------------------------------------------------------------------------

#[test]
fn fanout_rule_param_mismatch_errors() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_function_rule(&mut lang);
    merge_fanout_rule(&mut lang);
    // f: number ~> string, g: string ~> bool → params differ → unify errors.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("function").with_payload(json!({"children": [2, 3]})));
    ast.push(AstNode::new("fanout").with_payload(json!({"children": [4, 5]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        err.to_string().contains("unify") || err.to_string().contains("mismatch"),
        "expected unify error, got: {}",
        err
    );
}

/// Merge the canonical `product` (***) synth rule: synth both children,
/// extract param/body of each (no unification), construct
/// `type.function{param: type.array{elements: [A, C]},
///                 body:  type.array{elements: [B, D]}}`.
fn merge_product_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.product",
            "product",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("ftype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("gtype")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "type_field",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("ftype"),
                                    )],
                                    Value::from("param"),
                                ),
                                Scheme::seq(
                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("A")),
                                    Scheme::seq(
                                        Scheme::arr_judgment_lit(
                                            "type_field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("ftype"),
                                            )],
                                            Value::from("body"),
                                        ),
                                        Scheme::seq(
                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("B")),
                                            Scheme::seq(
                                                Scheme::arr_judgment_lit(
                                                    "type_field",
                                                    vec![Scheme::arr_judgment_lit(
                                                        "result",
                                                        vec![],
                                                        Value::from("gtype"),
                                                    )],
                                                    Value::from("param"),
                                                ),
                                                Scheme::seq(
                                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("C")),
                                                    Scheme::seq(
                                                        Scheme::arr_judgment_lit(
                                                            "type_field",
                                                            vec![Scheme::arr_judgment_lit(
                                                                "result",
                                                                vec![],
                                                                Value::from("gtype"),
                                                            )],
                                                            Value::from("body"),
                                                        ),
                                                        Scheme::seq(
                                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("D")),
                                                            Scheme::arr_judgment_lit(
                                                                "construct",
                                                                vec![
                                                                    Scheme::arr_judgment_lit(
                                                                        "field",
                                                                        vec![Scheme::arr_judgment_lit(
                                                                            "construct",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "field",
                                                                                vec![Scheme::arr_judgment(
                                                                                    "make_array",
                                                                                    vec![
                                                                                        Scheme::arr_judgment_lit(
                                                                                            "result",
                                                                                            vec![],
                                                                                            Value::from("A"),
                                                                                        ),
                                                                                        Scheme::arr_judgment_lit(
                                                                                            "result",
                                                                                            vec![],
                                                                                            Value::from("C"),
                                                                                        ),
                                                                                    ],
                                                                                )],
                                                                                Value::from("elements"),
                                                                            )],
                                                                            Value::from("type.array"),
                                                                        )],
                                                                        Value::from("param"),
                                                                    ),
                                                                    Scheme::arr_judgment_lit(
                                                                        "field",
                                                                        vec![Scheme::arr_judgment_lit(
                                                                            "construct",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "field",
                                                                                vec![Scheme::arr_judgment(
                                                                                    "make_array",
                                                                                    vec![
                                                                                        Scheme::arr_judgment_lit(
                                                                                            "result",
                                                                                            vec![],
                                                                                            Value::from("B"),
                                                                                        ),
                                                                                        Scheme::arr_judgment_lit(
                                                                                            "result",
                                                                                            vec![],
                                                                                            Value::from("D"),
                                                                                        ),
                                                                                    ],
                                                                                )],
                                                                                Value::from("elements"),
                                                                            )],
                                                                            Value::from("type.array"),
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
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
}

// ---------------------------------------------------------------------------
// product_rule_infers_arrow_kind_signature — `f *** g` where
// f: A ~> B, g: C ~> D ⟹ f *** g : (A, C) ~> (B, D)
// ---------------------------------------------------------------------------

#[test]
fn product_rule_infers_arrow_kind_signature() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_function_rule(&mut lang);
    merge_product_rule(&mut lang);
    // f: number ~> string, g: bool ~> number
    // → f *** g : (number, bool) ~> (string, number)
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("function").with_payload(json!({"children": [2, 3]})));
    ast.push(AstNode::new("product").with_payload(json!({"children": [4, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        type_function(
            type_array_elements(vec![type_number(), type_bool()]),
            type_array_elements(vec![type_string(), type_number()])
        )
    );
}

/// Merge the canonical `choice` (|||) synth rule: synth both children,
/// extract param/body of each, unify the two bodies, construct
/// `type.function{param: type.sum{variants: [A, B]}, body: C}`.
fn merge_choice_rule(lang: &mut Language) {
    lang.types
        .merge(TypeRuleDef::new(
            "type.choice",
            "choice",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("ftype")),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("bind", vec![], Value::from("gtype")),
                            Scheme::seq(
                                Scheme::arr_judgment_lit(
                                    "type_field",
                                    vec![Scheme::arr_judgment_lit(
                                        "result",
                                        vec![],
                                        Value::from("ftype"),
                                    )],
                                    Value::from("param"),
                                ),
                                Scheme::seq(
                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("A")),
                                    Scheme::seq(
                                        Scheme::arr_judgment_lit(
                                            "type_field",
                                            vec![Scheme::arr_judgment_lit(
                                                "result",
                                                vec![],
                                                Value::from("ftype"),
                                            )],
                                            Value::from("body"),
                                        ),
                                        Scheme::seq(
                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("C1")),
                                            Scheme::seq(
                                                Scheme::arr_judgment_lit(
                                                    "type_field",
                                                    vec![Scheme::arr_judgment_lit(
                                                        "result",
                                                        vec![],
                                                        Value::from("gtype"),
                                                    )],
                                                    Value::from("param"),
                                                ),
                                                Scheme::seq(
                                                    Scheme::arr_judgment_lit("bind", vec![], Value::from("B")),
                                                    Scheme::seq(
                                                        Scheme::arr_judgment_lit(
                                                            "type_field",
                                                            vec![Scheme::arr_judgment_lit(
                                                                "result",
                                                                vec![],
                                                                Value::from("gtype"),
                                                            )],
                                                            Value::from("body"),
                                                        ),
                                                        Scheme::seq(
                                                            Scheme::arr_judgment_lit("bind", vec![], Value::from("C2")),
                                                            Scheme::seq(
                                                                Scheme::arr_judgment(
                                                                    "unify",
                                                                    vec![
                                                                        Scheme::arr_judgment_lit(
                                                                            "result",
                                                                            vec![],
                                                                            Value::from("C1"),
                                                                        ),
                                                                        Scheme::arr_judgment_lit(
                                                                            "result",
                                                                            vec![],
                                                                            Value::from("C2"),
                                                                        ),
                                                                    ],
                                                                ),
                                                                Scheme::arr_judgment_lit(
                                                                    "construct",
                                                                    vec![
                                                                        Scheme::arr_judgment_lit(
                                                                            "field",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "construct",
                                                                                vec![Scheme::arr_judgment_lit(
                                                                                    "field",
                                                                                    vec![Scheme::arr_judgment(
                                                                                        "make_array",
                                                                                        vec![
                                                                                            Scheme::arr_judgment_lit(
                                                                                                "result",
                                                                                                vec![],
                                                                                                Value::from("A"),
                                                                                            ),
                                                                                            Scheme::arr_judgment_lit(
                                                                                                "result",
                                                                                                vec![],
                                                                                                Value::from("B"),
                                                                                            ),
                                                                                        ],
                                                                                    )],
                                                                                    Value::from("variants"),
                                                                                )],
                                                                                Value::from("type.sum"),
                                                                            )],
                                                                            Value::from("param"),
                                                                        ),
                                                                        Scheme::arr_judgment_lit(
                                                                            "field",
                                                                            vec![Scheme::arr_judgment_lit(
                                                                                "result",
                                                                                vec![],
                                                                                Value::from("C1"),
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
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ))
        .unwrap();
}

// ---------------------------------------------------------------------------
// choice_rule_infers_arrow_kind_signature — `f ||| g` where
// f: A ~> C, g: B ~> C ⟹ f ||| g : (A | B) ~> C
// ---------------------------------------------------------------------------

#[test]
fn choice_rule_infers_arrow_kind_signature() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_function_rule(&mut lang);
    merge_choice_rule(&mut lang);
    // f: number ~> string, g: bool ~> string
    // → f ||| g : (number | bool) ~> string
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("function").with_payload(json!({"children": [2, 3]})));
    ast.push(AstNode::new("choice").with_payload(json!({"children": [4, 5]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ,
        type_function(type_sum(vec![type_number(), type_bool()]), type_string())
    );
}

// ---------------------------------------------------------------------------
// choice_rule_body_mismatch_errors — `f ||| g` requires both arrows to
// share the same output type; a mismatch unifies and errors.
// ---------------------------------------------------------------------------

#[test]
fn choice_rule_body_mismatch_errors() {
    let engine = engine();
    let mut lang = Language::new();
    merge_value_rule(&mut lang);
    merge_function_rule(&mut lang);
    merge_choice_rule(&mut lang);
    // f: number ~> string, g: bool ~> number → bodies differ → unify errors.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hi")));
    ast.push(AstNode::new("value").with_payload(json!(true)));
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("function").with_payload(json!({"children": [2, 3]})));
    ast.push(AstNode::new("choice").with_payload(json!({"children": [4, 5]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        err.to_string().contains("unify") || err.to_string().contains("mismatch"),
        "expected unify error, got: {}",
        err
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
    // Child 0 (value 42) synths to type.int (the kind of the type param).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("forall").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.forall with kind=type.int and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.forall")
    );
    assert_eq!(result.typ.get("kind"), Some(&type_number()));
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
                            Scheme::arr_judgment_lit("result", vec![], Value::from("foralltype")),
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
    // Child 0 (value 42) synths to type.int (the param type).
    // Child 1 (value "hello") is the body child — its arena index (1) is
    // stored in the body_closure marker (NOT evaluated eagerly).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    ast.push(AstNode::new("vau").with_payload(json!({"children": [0, 1]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.vau with param=type.int and body_closure
    // pointing to child index 1.
    assert_eq!(
        result.typ.get("__tag__").and_then(|v| v.as_str()),
        Some("type.vau")
    );
    assert_eq!(result.typ.get("param"), Some(&type_number()));
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
                            Scheme::arr_judgment_lit("result", vec![], Value::from("vautype")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify Type_0 with Type_1")),
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
                Scheme::arr_judgment("universe_level", vec![Scheme::arr_native("id", vec![])]),
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
                Scheme::arr_judgment("universe_level", vec![Scheme::arr_native("id", vec![])]),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("check_refl: cannot prove")),
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
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify")),
        "expected unification failure, got {:?}",
        err
    );
}

// ===========================================================================
// ADR-0054 subtyping: universe cumulativity (Type_n ⊆ Type_m if n ≤ m)
// ===========================================================================

// ---------------------------------------------------------------------------
// subtype(type.number, type.number) → succeeds (same type)
// ---------------------------------------------------------------------------

#[test]
fn subtype_same_type_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct(type.number), construct(type.number))
    //       >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "subtype",
                    vec![
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                        Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
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
// subtype(Type_0, Type_1) → succeeds (cumulativity: lower ⊆ higher)
// ---------------------------------------------------------------------------

#[test]
fn subtype_universe_lower_to_higher_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct_universe(0), construct_universe(1))
    //       >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "subtype",
                    vec![
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(1u64)),
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
// subtype(Type_2, Type_2) → succeeds (same level)
// ---------------------------------------------------------------------------

#[test]
fn subtype_universe_same_level_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct_universe(2), construct_universe(2))
    //       >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "subtype",
                    vec![
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(2u64)),
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(2u64)),
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
// subtype(Type_1, Type_0) → fails (higher is not a subtype of lower)
// ---------------------------------------------------------------------------

#[test]
fn subtype_universe_higher_to_lower_fails() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct_universe(1), construct_universe(0)) → fails
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "subtype",
                vec![
                    Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(1u64)),
                    Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("Type_1 is not a subtype of Type_0")),
        "expected cumulativity failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// subtype(Type_0, Type_2) → succeeds (cumulativity across multiple levels)
// ---------------------------------------------------------------------------

#[test]
fn subtype_universe_zero_to_two_succeeds() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct_universe(0), construct_universe(2))
    //       >>> construct(type.string)
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment(
                    "subtype",
                    vec![
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(0u64)),
                        Scheme::arr_judgment_lit("construct_universe", vec![], Value::from(2u64)),
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
// subtype(type.number, type.string) → fails (different base types)
// ---------------------------------------------------------------------------

#[test]
fn subtype_different_base_types_fails() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: subtype(construct(type.number), construct(type.string)) → fails
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::arr_judgment(
                "subtype",
                vec![
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                    Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
                ],
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("test"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("is not a subtype of")),
        "expected subtyping failure, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// subtype(meta, type.number) → solves meta to type.number, succeeds
// ---------------------------------------------------------------------------

#[test]
fn subtype_meta_solves() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: fresh_meta >>> subtype(id, construct(type.number))
    //       >>> construct(type.string)
    // The subtype solves the fresh meta to type.number (no error). Result is
    // type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("fresh_meta", vec![], Value::Null),
                Scheme::seq(
                    Scheme::arr_judgment(
                        "subtype",
                        vec![
                            Scheme::arr_native("id", vec![]),
                            Scheme::arr_judgment_lit(
                                "construct",
                                vec![],
                                Value::from("type.number"),
                            ),
                        ],
                    ),
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

// ===========================================================================
// ADR-0074 ID.7 — Row polymorphism: open record types with row variables
// ===========================================================================

/// Build an open `type.record` value with the given fields and a row variable
/// (metavariable id) in the `rest` field.
fn type_record_open(fields: serde_json::Map<String, Value>, rest: &Value) -> Value {
    json!({
        "__tag__": "type.record",
        "__registry__": "handl.canonical",
        "fields": Value::Object(fields),
        "rest": rest.clone()
    })
}

// ---------------------------------------------------------------------------
// 1. Open record unifies with closed record with extra fields
//    {name: Str | r} ~ {name: Str, age: Int}  ⟹  r := {age: Int}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_open_unifies_closed_with_extra_fields() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    // Row variable r = meta 0.
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Open record: {name: Str | r}
    let mut open_fields = serde_json::Map::new();
    open_fields.insert("name".to_string(), type_string());
    let open_rec = type_record_open(open_fields, &r);
    // Closed record: {name: Str, age: Int}
    let mut closed_fields = serde_json::Map::new();
    closed_fields.insert("name".to_string(), type_string());
    closed_fields.insert("age".to_string(), type_number());
    let closed_rec = type_record(closed_fields);
    // Unify — should succeed and solve r to {age: Int}.
    engine.unify(&open_rec, &closed_rec, &mut ctx).unwrap();
    // Verify r was solved to type.record{fields: {age: type.number}}.
    let solved = ctx.lookup_meta(r_id).expect("row var should be solved");
    let mut expected_extra = serde_json::Map::new();
    expected_extra.insert("age".to_string(), type_number());
    assert_eq!(*solved, type_record(expected_extra));
}

// ---------------------------------------------------------------------------
// 2. Open record unifies with closed record with no extra fields
//    {name: Str | r} ~ {name: Str}  ⟹  r := {} (empty record)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_open_unifies_closed_no_extra_fields() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Open record: {name: Str | r}
    let mut open_fields = serde_json::Map::new();
    open_fields.insert("name".to_string(), type_string());
    let open_rec = type_record_open(open_fields, &r);
    // Closed record: {name: Str}
    let mut closed_fields = serde_json::Map::new();
    closed_fields.insert("name".to_string(), type_string());
    let closed_rec = type_record(closed_fields);
    // Unify — should succeed and solve r to {} (empty record).
    engine.unify(&open_rec, &closed_rec, &mut ctx).unwrap();
    let solved = ctx.lookup_meta(r_id).expect("row var should be solved");
    let empty_fields = serde_json::Map::new();
    assert_eq!(*solved, type_record(empty_fields));
}

// ---------------------------------------------------------------------------
// 3. Two open records unify (same fields)
//    {name: Str | r1} ~ {name: Str | r2}  ⟹  r1 ~ r2 (linked)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_two_open_records_unify() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r1_id = ctx.fresh_meta();
    let r1 = meta_value(r1_id);
    let r2_id = ctx.fresh_meta();
    let r2 = meta_value(r2_id);
    // Open record A: {name: Str | r1}
    let mut fields_a = serde_json::Map::new();
    fields_a.insert("name".to_string(), type_string());
    let rec_a = type_record_open(fields_a, &r1);
    // Open record B: {name: Str | r2}
    let mut fields_b = serde_json::Map::new();
    fields_b.insert("name".to_string(), type_string());
    let rec_b = type_record_open(fields_b, &r2);
    // Unify — should succeed. r1 and r2 are linked (one solves to the other).
    engine.unify(&rec_a, &rec_b, &mut ctx).unwrap();
    // At least one of r1/r2 should be solved to the other's meta.
    let solved1 = ctx.lookup_meta(r1_id);
    let solved2 = ctx.lookup_meta(r2_id);
    assert!(
        solved1.is_some() || solved2.is_some(),
        "at least one row var should be solved"
    );
}

// ---------------------------------------------------------------------------
// 4. Open records with different fields
//    {name: Str, age: Int | r1} ~ {name: Str, city: Str | r2}
//    ⟹  name unifies, r1 := {city: Str | r3}, r2 := {age: Int | r3}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_open_records_different_fields() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r1_id = ctx.fresh_meta();
    let r1 = meta_value(r1_id);
    let r2_id = ctx.fresh_meta();
    let r2 = meta_value(r2_id);
    // Open record A: {name: Str, age: Int | r1}
    let mut fields_a = serde_json::Map::new();
    fields_a.insert("name".to_string(), type_string());
    fields_a.insert("age".to_string(), type_number());
    let rec_a = type_record_open(fields_a, &r1);
    // Open record B: {name: Str, city: Str | r2}
    let mut fields_b = serde_json::Map::new();
    fields_b.insert("name".to_string(), type_string());
    fields_b.insert("city".to_string(), type_string());
    let rec_b = type_record_open(fields_b, &r2);
    // Unify — should succeed.
    engine.unify(&rec_a, &rec_b, &mut ctx).unwrap();
    // r1 should be solved to {city: Str | r3} where r3 is a fresh meta.
    let solved1 = ctx.lookup_meta(r1_id).expect("r1 should be solved");
    let solved1_tag = solved1.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(solved1_tag, Some("type.record"));
    let solved1_fields = solved1.get("fields").and_then(|f| f.as_object()).unwrap();
    assert!(solved1_fields.contains_key("city"));
    assert_eq!(solved1_fields.get("city"), Some(&type_string()));
    assert!(
        solved1.get("rest").is_some(),
        "r1's solution should have a rest (fresh row var)"
    );
    // r2 should be solved to {age: Int | r3} (same fresh meta).
    let solved2 = ctx.lookup_meta(r2_id).expect("r2 should be solved");
    let solved2_tag = solved2.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(solved2_tag, Some("type.record"));
    let solved2_fields = solved2.get("fields").and_then(|f| f.as_object()).unwrap();
    assert!(solved2_fields.contains_key("age"));
    assert_eq!(solved2_fields.get("age"), Some(&type_number()));
    assert!(
        solved2.get("rest").is_some(),
        "r2's solution should have a rest (fresh row var)"
    );
    // The rest (fresh row var r3) should be the same in both solutions.
    let rest1 = solved1.get("rest").unwrap();
    let rest2 = solved2.get("rest").unwrap();
    assert_eq!(
        rest1, rest2,
        "both solutions should share the same fresh row var"
    );
}

// ---------------------------------------------------------------------------
// 5. Closed records with same fields unify (regression — no row vars)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_closed_records_same_fields_unify() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    fields.insert("age".to_string(), type_number());
    let rec_a = type_record(fields.clone());
    let rec_b = type_record(fields);
    engine.unify(&rec_a, &rec_b, &mut ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 6. Closed records with different fields fail to unify
// ---------------------------------------------------------------------------

#[test]
fn row_poly_closed_records_different_fields_fail() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let mut fields_a = serde_json::Map::new();
    fields_a.insert("name".to_string(), type_string());
    let rec_a = type_record(fields_a);
    let mut fields_b = serde_json::Map::new();
    fields_b.insert("name".to_string(), type_string());
    fields_b.insert("age".to_string(), type_number());
    let rec_b = type_record(fields_b);
    let err = engine.unify(&rec_a, &rec_b, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify closed records")),
        "expected closed-record mismatch, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 7. Open record with known field absent from closed record fails
//    {name: Str, age: Int | r} ~ {name: Str}  ⟹  FAIL (age required but absent)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_open_requires_field_absent_from_closed_fails() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Open record: {name: Str, age: Int | r}
    let mut open_fields = serde_json::Map::new();
    open_fields.insert("name".to_string(), type_string());
    open_fields.insert("age".to_string(), type_number());
    let open_rec = type_record_open(open_fields, &r);
    // Closed record: {name: Str} — no age field.
    let mut closed_fields = serde_json::Map::new();
    closed_fields.insert("name".to_string(), type_string());
    let closed_rec = type_record(closed_fields);
    let err = engine.unify(&open_rec, &closed_rec, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("open requires fields")),
        "expected open-requires-field error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 8. Field access on an open record extracts the known field type
//    record_field_type({name: Str | r}, "name") → Str
// ---------------------------------------------------------------------------

#[test]
fn row_poly_field_access_on_open_record() {
    let engine = engine();
    let mut lang = Language::new();
    // Build an open record type value with {name: Str | r}.
    let r = json!({"__meta__": 0});
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    let open_rec = type_record_open(fields, &r);
    // Rule: const(open_rec) >>> bind("rt")
    //   >>> record_field_type(result("rt"), lit="name")
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], open_rec),
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
// 9. check_object_record with open expected type succeeds with extra fields
//    Check {name: "Luis", age: 30} against {name: Str | r} → succeeds, r := {age: Int}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_check_object_record_open_expected() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
    // Create a fresh row var for the expected type.
    // We use a fixed meta id 0 (the engine allocates metas from 0).
    let r = json!({"__meta__": 0});
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    let open_rec = type_record_open(fields, &r);
    // Wrapper rule: check child 0 against the open record, return type.string.
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit("const", vec![], open_rec)],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    // AST: [object{entries: [{key:"name", value:"Luis"},
    //                        {key:"age", value:30}]},
    //       wrapper{children:[0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "Luis"},
            {"key": "age", "value": 30}
        ]
    })));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 10. check_object_record with closed expected type rejects extra fields
//     Check {name: "Luis", age: 30} against {name: Str} → fails (age not expected)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_check_object_record_closed_rejects_extra() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    let closed_rec = type_record(fields);
    lang.types
        .merge(TypeRuleDef::new(
            "type.wrapper",
            "wrapper",
            Scheme::seq(
                Scheme::arr_judgment_lit(
                    "check_child",
                    vec![Scheme::arr_native_lit("const", vec![], closed_rec)],
                    Value::from(0u64),
                ),
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.string")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "Luis"},
            {"key": "age", "value": 30}
        ]
    })));
    ast.push(AstNode::new("wrapper").with_payload(json!({"children": [0]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("not found in expected record")),
        "expected field-not-found error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 11. fresh_row_meta judgment op creates a metavariable
// ---------------------------------------------------------------------------

#[test]
fn fresh_row_meta_creates_meta() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: fresh_row_meta >>> construct(type.string)
    // The fresh_row_meta should produce {"__meta__": 0}.
    lang.types
        .merge(TypeRuleDef::new(
            "type.test",
            "test",
            Scheme::seq(
                Scheme::arr_judgment_lit("fresh_row_meta", vec![], Value::Null),
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
// 12. Open record unifies with closed record (reversed argument order)
//     {name: Str, age: Int} ~ {name: Str | r}  ⟹  r := {age: Int}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_closed_unifies_open_reversed() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Closed record: {name: Str, age: Int}
    let mut closed_fields = serde_json::Map::new();
    closed_fields.insert("name".to_string(), type_string());
    closed_fields.insert("age".to_string(), type_number());
    let closed_rec = type_record(closed_fields);
    // Open record: {name: Str | r}
    let mut open_fields = serde_json::Map::new();
    open_fields.insert("name".to_string(), type_string());
    let open_rec = type_record_open(open_fields, &r);
    // Unify with closed first, open second.
    engine.unify(&closed_rec, &open_rec, &mut ctx).unwrap();
    let solved = ctx.lookup_meta(r_id).expect("row var should be solved");
    let mut expected_extra = serde_json::Map::new();
    expected_extra.insert("age".to_string(), type_number());
    assert_eq!(*solved, type_record(expected_extra));
}

// ---------------------------------------------------------------------------
// 13. Occurs check: a row variable can't be bound to a row containing itself
//     unify(r, {name: Str | r})  ⟹  FAIL (occurs-check)
// ---------------------------------------------------------------------------

#[test]
fn row_poly_occurs_check_rejects_cyclic_row() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Build a record that contains r in its own rest field:
    // {name: Str | r}  — unifying r with this would create a cycle.
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), type_string());
    let cyclic_rec = type_record_open(fields, &r);
    // Unify r with the cyclic record — should fail the occurs check.
    let err = engine.unify(&r, &cyclic_rec, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("occurs-check")),
        "expected occurs-check failure, got {:?}",
        err
    );
    // The row variable should remain unsolved.
    assert!(
        ctx.lookup_meta(r_id).is_none(),
        "row var should remain unsolved"
    );
}

// ---------------------------------------------------------------------------
// 14. Occurs check through nested row: r can't be bound to {a: Str | {b: Int | r}}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_occurs_check_nested_row() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    // Inner record: {b: Int | r}  — contains r.
    let mut inner_fields = serde_json::Map::new();
    inner_fields.insert("b".to_string(), type_number());
    let inner_rec = type_record_open(inner_fields, &r);
    // Outer record: {a: Str | inner_rec}  — r occurs nested.
    let mut outer_fields = serde_json::Map::new();
    outer_fields.insert("a".to_string(), type_string());
    let cyclic_rec = type_record_open(outer_fields, &inner_rec);
    // Unify r with the nested cyclic record — should fail the occurs check.
    let err = engine.unify(&r, &cyclic_rec, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("occurs-check")),
        "expected occurs-check failure for nested row, got {:?}",
        err
    );
    assert!(
        ctx.lookup_meta(r_id).is_none(),
        "row var should remain unsolved"
    );
}

// ---------------------------------------------------------------------------
// 15. Function application with row-polymorphic record subsumption
//     A function expecting {name: Str | r} accepts {name: "L", age: 30}
//     via the `call` rule's check_child → check_object_record path.
//     The row variable r is solved to {age: Int}.
// ---------------------------------------------------------------------------

#[test]
fn row_poly_function_call_subsumption() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
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
    // "fn_val" node: always synths to a fixed function type with an open
    // record param: type.function{param: {name: Str | r}, body: Str}
    // where r = meta 0.
    let r = json!({"__meta__": 0});
    let mut param_fields = serde_json::Map::new();
    param_fields.insert("name".to_string(), type_string());
    let open_param = type_record_open(param_fields, &r);
    let fn_type = json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": open_param,
        "body": type_string()
    });
    lang.types
        .merge(TypeRuleDef::new(
            "type.fn_val",
            "fn_val",
            Scheme::arr_native_lit("const", vec![], fn_type),
        ))
        .unwrap();
    // AST: [object{entries: [{key:"name", value:"Luis"},
    //                        {key:"age", value:30}]},
    //       fn_val,
    //       call{children:[1, 0]}]
    // The call node synths the callee (fn_val) → type.function{param: {name:Str|r}, body: Str},
    // checks the object arg against {name: Str | r} → check_object_record succeeds
    // (extra "age" field absorbed by r), returns body type Str.
    let mut ast = Ast::new();
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "Luis"},
            {"key": "age", "value": 30}
        ]
    })));
    ast.push(AstNode::new("fn_val"));
    ast.push(AstNode::new("call").with_payload(json!({"children": [1, 0]})));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_string());
}

// ---------------------------------------------------------------------------
// 16. Function call subsumption fails with closed expected record + extra fields
//     A function expecting {name: Str} (closed) rejects {name: "L", age: 30}.
// ---------------------------------------------------------------------------

#[test]
fn row_poly_function_call_closed_rejects_extra() {
    let engine = engine();
    let mut lang = Language::new();
    merge_object_rule(&mut lang);
    // call rule (same as above).
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
    // "fn_val" node: synths to a function with a CLOSED record param.
    let mut param_fields = serde_json::Map::new();
    param_fields.insert("name".to_string(), type_string());
    let closed_param = type_record(param_fields);
    let fn_type = json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": closed_param,
        "body": type_string()
    });
    lang.types
        .merge(TypeRuleDef::new(
            "type.fn_val",
            "fn_val",
            Scheme::arr_native_lit("const", vec![], fn_type),
        ))
        .unwrap();
    // AST: [object{name:"Luis", age:30}, fn_val, call{children:[1,0]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "name", "value": "Luis"},
            {"key": "age", "value": 30}
        ]
    })));
    ast.push(AstNode::new("fn_val"));
    ast.push(AstNode::new("call").with_payload(json!({"children": [1, 0]})));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("not found in expected record")),
        "expected field-not-found error for closed record, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 17. Row variable solved through unify with a record containing a row var
//     {name: Str | r1} ~ {name: Str, age: Int | r2}  ⟹  r1 := {age: Int | r2}
// ---------------------------------------------------------------------------

#[test]
fn row_poly_open_unifies_open_with_extra_known_field() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r1_id = ctx.fresh_meta();
    let r1 = meta_value(r1_id);
    let r2_id = ctx.fresh_meta();
    let r2 = meta_value(r2_id);
    // Open record A: {name: Str | r1}
    let mut fields_a = serde_json::Map::new();
    fields_a.insert("name".to_string(), type_string());
    let rec_a = type_record_open(fields_a, &r1);
    // Open record B: {name: Str, age: Int | r2}
    let mut fields_b = serde_json::Map::new();
    fields_b.insert("name".to_string(), type_string());
    fields_b.insert("age".to_string(), type_number());
    let rec_b = type_record_open(fields_b, &r2);
    // Unify — should succeed. Per the Leijen row-unification algorithm
    // (both-open case), a fresh row variable r3 is created and:
    //   r1 := {age: Int | r3},  r2 := {} | r3
    // (r2 absorbs a's extra fields — none — plus the shared fresh row var.)
    engine.unify(&rec_a, &rec_b, &mut ctx).unwrap();
    let solved1 = ctx.lookup_meta(r1_id).expect("r1 should be solved");
    let solved1_tag = solved1.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(solved1_tag, Some("type.record"));
    let solved1_fields = solved1.get("fields").and_then(|f| f.as_object()).unwrap();
    assert_eq!(solved1_fields.get("age"), Some(&type_number()));
    // r1's solution should carry a fresh row variable as its rest.
    let rest1 = solved1.get("rest").expect("r1 solution should have a rest");
    let rest1_meta = rest1.get("__meta__").and_then(|m| m.as_u64());
    assert!(
        rest1_meta.is_some(),
        "r1's rest should be a fresh row var (meta)"
    );
    // r2 should also be solved (to {} | r3, sharing the same fresh row var).
    let solved2 = ctx.lookup_meta(r2_id).expect("r2 should be solved");
    let solved2_tag = solved2.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(solved2_tag, Some("type.record"));
    let solved2_fields = solved2.get("fields").and_then(|f| f.as_object()).unwrap();
    assert!(
        solved2_fields.is_empty(),
        "r2's fields should be empty (a had no extra fields)"
    );
    let rest2 = solved2.get("rest").expect("r2 solution should have a rest");
    // Both solutions should share the same fresh row variable.
    assert_eq!(
        rest1, rest2,
        "both solutions should share the same fresh row var"
    );
}

// ===========================================================================
// synth_rec — rec region fixpoint typing (ADR-0074 ID.3, Paterson loop)
// ===========================================================================

/// Build a [`Language`] with the type rules needed to typecheck `rec`
/// regions: `value`, `var`, `if`, the numeric/comparison binary ops,
/// `return`, `let`, `rec`, and a `fresh` node (whose type rule mints a fresh
/// metavariable on every synth — used by the fuel-limit test). The rules
/// mirror the canonical `types.yaml` schemes so the `synth_rec` judgment can
/// synth the binding value expressions and the result expression.
fn rec_test_lang() -> Language {
    let mut lang = Language::new();
    // value: node_payload >>> json_type_construct
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
    // var: node_payload >>> lookup
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
    // if: synth cond, unify with type.bool, synth then, unify with else,
    // return then-type (the canonical `if` synth rule).
    lang.types
        .merge(TypeRuleDef::new(
            "type.if",
            "if",
            Scheme::seq(
                Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                Scheme::seq(
                    Scheme::arr_judgment_lit("bind", vec![], Value::from("tcond")),
                    Scheme::seq(
                        Scheme::arr_judgment(
                            "unify",
                            vec![
                                Scheme::arr_judgment_lit("result", vec![], Value::from("tcond")),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.bool"),
                                ),
                            ],
                        ),
                        Scheme::seq(
                            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(1u64)),
                            Scheme::seq(
                                Scheme::arr_judgment_lit("bind", vec![], Value::from("tthen")),
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
        ))
        .unwrap();
    // Numeric binary ops (add/sub): synth child 0, unify with type.number,
    // synth child 1, unify with type.number, return type.number.
    for kind in ["binary.add", "binary.sub"] {
        lang.types
            .merge(TypeRuleDef::new(
                format!("type.{}", kind),
                kind,
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
                        Scheme::seq(
                            Scheme::arr_judgment(
                                "unify",
                                vec![
                                    Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                                    Scheme::arr_judgment_lit(
                                        "construct",
                                        vec![],
                                        Value::from("type.number"),
                                    ),
                                ],
                            ),
                            Scheme::seq(
                                Scheme::arr_judgment(
                                    "unify",
                                    vec![
                                        Scheme::arr_judgment_lit(
                                            "synth_child",
                                            vec![],
                                            Value::from(1u64),
                                        ),
                                        Scheme::arr_judgment_lit(
                                            "construct",
                                            vec![],
                                            Value::from("type.number"),
                                        ),
                                    ],
                                ),
                                Scheme::arr_judgment_lit(
                                    "construct",
                                    vec![],
                                    Value::from("type.number"),
                                ),
                            ),
                        ),
                    ),
                ),
            ))
            .unwrap();
    }
    // Comparison binary ops (gt/lt): synth child 0, unify with child 1,
    // return type.bool.
    for kind in ["binary.gt", "binary.lt"] {
        lang.types
            .merge(TypeRuleDef::new(
                format!("type.{}", kind),
                kind,
                Scheme::seq(
                    Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
                    Scheme::seq(
                        Scheme::arr_judgment_lit("bind", vec![], Value::from("a")),
                        Scheme::seq(
                            Scheme::arr_judgment(
                                "unify",
                                vec![
                                    Scheme::arr_judgment_lit("result", vec![], Value::from("a")),
                                    Scheme::arr_judgment_lit(
                                        "synth_child",
                                        vec![],
                                        Value::from(1u64),
                                    ),
                                ],
                            ),
                            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
                        ),
                    ),
                ),
            ))
            .unwrap();
    }
    // return: synth child 0 (the return value).
    lang.types
        .merge(TypeRuleDef::new(
            "type.return",
            "return",
            Scheme::arr_judgment_lit("synth_child", vec![], Value::from(0u64)),
        ))
        .unwrap();
    // let: delegate to the synth_let judgment (handles the v0.1
    // {name, children:[value, body]} shape used in the countdown test).
    lang.types
        .merge(TypeRuleDef::new(
            "type.let",
            "let",
            Scheme::arr_judgment("synth_let", vec![]),
        ))
        .unwrap();
    // rec: delegate to the synth_rec judgment (fixpoint typing).
    lang.types
        .merge(TypeRuleDef::new(
            "type.rec",
            "rec",
            Scheme::arr_judgment("synth_rec", vec![]),
        ))
        .unwrap();
    // fresh: mints a fresh metavariable on every synth (used by the
    // fuel-limit test to construct a rec that never converges).
    lang.types
        .merge(TypeRuleDef::new(
            "type.fresh",
            "fresh",
            Scheme::arr_judgment("fresh_meta", vec![]),
        ))
        .unwrap();
    // AST shape roles: assign has target/value, return has value.
    lang.ast
        .merge(AstShapeDef::new("ast.assign", "assign").with_children(vec![
            ChildRoleSpec {
                role: "target".into(),
                variadic: false,
            },
            ChildRoleSpec {
                role: "value".into(),
                variadic: false,
            },
        ]))
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.return", "return").with_children(vec![ChildRoleSpec {
                role: "value".into(),
                variadic: false,
            }]),
        )
        .unwrap();
    lang
}

// ---------------------------------------------------------------------------
// 1. Simple fixpoint: rec { x = 5; x } → type.number (no cycle)
// ---------------------------------------------------------------------------

#[test]
fn rec_simple_fixpoint_is_int() {
    let engine = engine();
    let lang = rec_test_lang();
    // AST: [value(5), var("x"), assign{[1,0]}, var("x"), rec{[2,3]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(5))); // 0
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 1: LHS
    ast.push(AstNode::new("assign").with_payload(json!({"children": [1, 0]}))); // 2: x = 5
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 3: result
    ast.push(AstNode::new("rec").with_payload(json!({"children": [2, 3]}))); // 4: root
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// 2. Cyclic fixpoint: rec { a = b; b = a } → shared type variable
// ---------------------------------------------------------------------------

#[test]
fn rec_cyclic_fixpoint_converges_to_shared_type_var() {
    let engine = engine();
    let lang = rec_test_lang();
    // AST: [var("b"), var("a"), assign{[1,0]} (a=b),
    //       var("a"), var("b"), assign{[4,3]} (b=a), rec{[2,5]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("b"))); // 0: b in a=b
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 1: LHS a
    ast.push(AstNode::new("assign").with_payload(json!({"children": [1, 0]}))); // 2: a = b
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 3: a in b=a
    ast.push(AstNode::new("var").with_payload(json!("b"))); // 4: LHS b
    ast.push(AstNode::new("assign").with_payload(json!({"children": [4, 3]}))); // 5: b = a
    ast.push(AstNode::new("rec").with_payload(json!({"children": [2, 5]}))); // 6: root
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The fixpoint is a shared, unsolved metavariable (type variable). Both
    // bindings converge to the same meta; the result is that type variable.
    assert!(
        extract_meta_id(&result.typ).is_some(),
        "expected a shared type variable (metavariable), got {:?}",
        result.typ
    );
}

// ---------------------------------------------------------------------------
// 3. Countdown: let n = 5 in rec { n = if n > 0 then n - 1 else n; n } → Int
// ---------------------------------------------------------------------------

#[test]
fn rec_countdown_is_int() {
    let engine = engine();
    let lang = rec_test_lang();
    // Build the if expression: if n > 0 then n - 1 else n
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(5))); // 0: n initial
    // n > 0
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 1: n in n>0
    ast.push(AstNode::new("value").with_payload(json!(0))); // 2: 0 in n>0
    ast.push(AstNode::new("binary.gt").with_payload(json!({"children": [1, 2]}))); // 3: n>0
    // n - 1
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 4: n in n-1
    ast.push(AstNode::new("value").with_payload(json!(1))); // 5: 1 in n-1
    ast.push(AstNode::new("binary.sub").with_payload(json!({"children": [4, 5]}))); // 6: n-1
    // else: n
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 7: n (else)
    ast.push(AstNode::new("if").with_payload(json!({"children": [3, 6, 7]}))); // 8: if ...
    // n = if ...
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 9: LHS n
    ast.push(AstNode::new("assign").with_payload(json!({"children": [9, 8]}))); // 10: n = if ...
    // result: n
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 11: result
    // rec { n = if ...; n }
    ast.push(AstNode::new("rec").with_payload(json!({"children": [10, 11]}))); // 12: rec
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 4. Mutual dependency: rec { a = b + 1; b = 0; a } → Int
// ---------------------------------------------------------------------------

#[test]
fn rec_mutual_dependency_is_int() {
    let engine = engine();
    let lang = rec_test_lang();
    let mut ast = Ast::new();
    // a = b + 1
    ast.push(AstNode::new("var").with_payload(json!("b"))); // 0: b in b+1
    ast.push(AstNode::new("value").with_payload(json!(1))); // 1: 1
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]}))); // 2: b+1
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 3: LHS a
    ast.push(AstNode::new("assign").with_payload(json!({"children": [3, 2]}))); // 4: a = b+1
    // b = 0
    ast.push(AstNode::new("value").with_payload(json!(0))); // 5: 0
    ast.push(AstNode::new("var").with_payload(json!("b"))); // 6: LHS b
    ast.push(AstNode::new("assign").with_payload(json!({"children": [6, 5]}))); // 7: b = 0
    // result: a
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 8: result
    ast.push(AstNode::new("rec").with_payload(json!({"children": [4, 7, 8]}))); // 9: root
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 5. Fuel limit: a rec that never converges in types errors (does not hang)
// ---------------------------------------------------------------------------

#[test]
fn rec_fuel_limit_errors() {
    let engine = engine();
    let lang = rec_test_lang();
    // rec { a = fresh; a } — `fresh` mints a new metavariable on every synth.
    // Each fixpoint pass solves the binding's meta to a brand-new meta, so
    // the zonked binding type changes every pass and never stabilizes. The
    // fuel limit (100) must fire and produce a compile error, not hang.
    let mut ast = Ast::new();
    ast.push(AstNode::new("fresh")); // 0: fresh (type = fresh meta each synth)
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 1: LHS a
    ast.push(AstNode::new("assign").with_payload(json!({"children": [1, 0]}))); // 2: a = fresh
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 3: result
    ast.push(AstNode::new("rec").with_payload(json!({"children": [2, 3]}))); // 4: root
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot determine fixpoint type for rec region")),
        "expected fixpoint fuel-limit error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 6. Return type: rec { x = 42; return x } → Int
// ---------------------------------------------------------------------------

#[test]
fn rec_return_type_is_int() {
    let engine = engine();
    let lang = rec_test_lang();
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42))); // 0: lit 42
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 1: LHS x
    ast.push(AstNode::new("assign").with_payload(json!({"children": [1, 0]}))); // 2: x = 42
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 3: return value
    ast.push(AstNode::new("return").with_payload(json!({"children": [3]}))); // 4: return x
    ast.push(AstNode::new("rec").with_payload(json!({"children": [2, 4]}))); // 5: root
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ---------------------------------------------------------------------------
// 7. Empty rec region → type.unit
// ---------------------------------------------------------------------------

#[test]
fn rec_empty_region_is_unit() {
    let engine = engine();
    let lang = rec_test_lang();
    let mut ast = Ast::new();
    ast.push(AstNode::new("rec").with_payload(json!({"children": []}))); // 0: root
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_unit());
}

// ---------------------------------------------------------------------------
// 8. Occurs-check rejects a genuinely cyclic (infinite) type:
//    rec { a = [a]; a } — a's type would be Array(a), an infinite type.
// ---------------------------------------------------------------------------

#[test]
fn rec_occurs_check_rejects_cyclic_type() {
    let engine = engine();
    let lang = rec_test_lang();
    // We need an array node whose element type is `a`. Add an `array` rule
    // that constructs type.array{element: synth_first_child_or_unknown}.
    let mut lang = lang;
    lang.types
        .merge(TypeRuleDef::new(
            "type.array",
            "array",
            Scheme::arr_judgment_lit(
                "construct",
                vec![Scheme::arr_judgment_lit(
                    "field",
                    vec![Scheme::arr_judgment("synth_first_child_or_unknown", vec![])],
                    Value::from("element"),
                )],
                Value::from("type.array"),
            ),
        ))
        .unwrap();
    // AST: [var("a"), array{[0]}, var("a"), assign{[2,1]} (a = [a]),
    //       var("a"), rec{[3,4]}]
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 0: a in [a]
    ast.push(AstNode::new("array").with_payload(json!({"children": [0]}))); // 1: [a]
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 2: LHS a
    ast.push(AstNode::new("assign").with_payload(json!({"children": [2, 1]}))); // 3: a = [a]
    ast.push(AstNode::new("var").with_payload(json!("a"))); // 4: result
    ast.push(AstNode::new("rec").with_payload(json!({"children": [3, 4]}))); // 5: root
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("occurs-check failed")),
        "expected occurs-check failure for cyclic type, got {:?}",
        err
    );
}

// ===========================================================================
// ADR-0054 / ADR-0076 — Effect row tracking: type.effects, infer_effect_row
// wiring, and row-polymorphic effect row unification (unify_effects).
// ===========================================================================

/// Build a closed `type.effects` value with the given ops (sorted) and
/// `rest: null`.
fn type_effects_closed(ops: &[&str]) -> Value {
    let ops_arr: Vec<Value> = ops.iter().map(|s| Value::from(*s)).collect();
    json!({
        "__tag__": "type.effects",
        "__registry__": "handl.canonical",
        "ops": ops_arr,
        "rest": Value::Null
    })
}

/// Build an open `type.effects` value with the given ops and a row variable
/// (metavariable) in the `rest` field.
fn type_effects_open(ops: &[&str], rest: &Value) -> Value {
    let ops_arr: Vec<Value> = ops.iter().map(|s| Value::from(*s)).collect();
    json!({
        "__tag__": "type.effects",
        "__registry__": "handl.canonical",
        "ops": ops_arr,
        "rest": rest.clone()
    })
}

/// Build a `type.function` value with an `effects` field.
fn type_function_with_effects(param: Value, body: Value, effects: Value) -> Value {
    json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": param,
        "body": body,
        "effects": effects
    })
}

// ---------------------------------------------------------------------------
// 1. infer_effect_row of a Handle scheme produces a type.effects value with
//    the right ops, attached to the resulting type.function.
// ---------------------------------------------------------------------------

#[test]
fn effect_row_handle_attaches_effects_to_function_type() {
    let engine = engine();
    let mut lang = Language::new();
    // A type rule whose scheme is a Handle. The body constructs
    // type.function{param: type.number, body: type.string}; the target is
    // "map"; the impl is a trivial const(type.number). infer_effect_row of
    // this scheme is {map} (the Handle target). After elimination the body
    // is unchanged (no bare arr("map") references), so the result is
    // type.function{...} with an attached effects field.
    lang.types
        .merge(TypeRuleDef::new(
            "type.handler",
            "handler",
            Scheme::handle(
                Scheme::arr_judgment_lit(
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
                ),
                "map",
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("handler"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.function with param=type.number,
    // body=type.string, and effects=type.effects{ops:["map"], rest:null}.
    assert_eq!(
        result.typ.get("__tag__").and_then(|t| t.as_str()),
        Some("type.function")
    );
    assert_eq!(result.typ.get("param"), Some(&type_number()));
    assert_eq!(result.typ.get("body"), Some(&type_string()));
    let effects = result
        .typ
        .get("effects")
        .expect("effects field should be present");
    assert_eq!(
        effects.get("__tag__").and_then(|t| t.as_str()),
        Some("type.effects")
    );
    let ops = effects.get("ops").and_then(|o| o.as_array()).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0], Value::from("map"));
    assert_eq!(effects.get("rest"), Some(&Value::Null));
}

// ---------------------------------------------------------------------------
// 2. Unifying two closed effect rows with the same ops succeeds.
// ---------------------------------------------------------------------------

#[test]
fn unify_effects_closed_same_ops_succeeds() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let a = type_effects_closed(&["map", "serialize"]);
    let b = type_effects_closed(&["map", "serialize"]);
    engine.unify(&a, &b, &mut ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 3. Unifying two closed effect rows with different ops fails.
// ---------------------------------------------------------------------------

#[test]
fn unify_effects_closed_different_ops_fails() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let a = type_effects_closed(&["map"]);
    let b = type_effects_closed(&["map", "serialize"]);
    let err = engine.unify(&a, &b, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify closed effect rows")),
        "expected closed-effect-row mismatch, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 4. Unifying an open row with a closed row: the closed row's extra ops are
//    assigned to the open row's variable.
//    {map | r} ~ {map, serialize}  ⟹  r := {serialize}
// ---------------------------------------------------------------------------

#[test]
fn unify_effects_open_with_closed_assigns_extra_ops() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    let open = type_effects_open(&["map"], &r);
    let closed = type_effects_closed(&["map", "serialize"]);
    engine.unify(&open, &closed, &mut ctx).unwrap();
    // r should be solved to type.effects{ops:["serialize"], rest:null}.
    let solved = ctx.lookup_meta(r_id).expect("row var should be solved");
    assert_eq!(
        solved.get("__tag__").and_then(|t| t.as_str()),
        Some("type.effects")
    );
    let solved_ops = solved.get("ops").and_then(|o| o.as_array()).unwrap();
    assert_eq!(solved_ops.len(), 1);
    assert_eq!(solved_ops[0], Value::from("serialize"));
    assert_eq!(solved.get("rest"), Some(&Value::Null));
}

// ---------------------------------------------------------------------------
// 5. Unifying an open row with a closed row where the open row has an op
//    absent from the closed row fails.
//    {map, log | r} ~ {map}  ⟹  FAIL (log required but absent)
// ---------------------------------------------------------------------------

#[test]
fn unify_effects_open_requires_op_absent_from_closed_fails() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r_id = ctx.fresh_meta();
    let r = meta_value(r_id);
    let open = type_effects_open(&["map", "log"], &r);
    let closed = type_effects_closed(&["map"]);
    let err = engine.unify(&open, &closed, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("open requires ops")),
        "expected open-requires-ops error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 6. Two open effect rows unify (different ops) via a shared fresh row var.
//    {map | r1} ~ {serialize | r2}
//    ⟹  r1 := {serialize | r3}, r2 := {map | r3}
// ---------------------------------------------------------------------------

#[test]
fn unify_effects_two_open_different_ops() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let r1_id = ctx.fresh_meta();
    let r1 = meta_value(r1_id);
    let r2_id = ctx.fresh_meta();
    let r2 = meta_value(r2_id);
    let a = type_effects_open(&["map"], &r1);
    let b = type_effects_open(&["serialize"], &r2);
    engine.unify(&a, &b, &mut ctx).unwrap();
    // r1 should be solved to type.effects{ops:["serialize"], rest:<fresh>}.
    let solved1 = ctx.lookup_meta(r1_id).expect("r1 should be solved");
    assert_eq!(
        solved1.get("__tag__").and_then(|t| t.as_str()),
        Some("type.effects")
    );
    let solved1_ops = solved1.get("ops").and_then(|o| o.as_array()).unwrap();
    assert_eq!(solved1_ops.len(), 1);
    assert_eq!(solved1_ops[0], Value::from("serialize"));
    assert!(
        solved1.get("rest").is_some(),
        "r1's solution should have a rest"
    );
    // r2 should be solved to type.effects{ops:["map"], rest:<fresh>}.
    let solved2 = ctx.lookup_meta(r2_id).expect("r2 should be solved");
    assert_eq!(
        solved2.get("__tag__").and_then(|t| t.as_str()),
        Some("type.effects")
    );
    let solved2_ops = solved2.get("ops").and_then(|o| o.as_array()).unwrap();
    assert_eq!(solved2_ops.len(), 1);
    assert_eq!(solved2_ops[0], Value::from("map"));
    assert!(
        solved2.get("rest").is_some(),
        "r2's solution should have a rest"
    );
    // Both solutions should share the same fresh row variable.
    let rest1 = solved1.get("rest").unwrap();
    let rest2 = solved2.get("rest").unwrap();
    assert_eq!(
        rest1, rest2,
        "both solutions should share the same fresh row var"
    );
}

// ---------------------------------------------------------------------------
// 7. A function type with an effects field round-trips through the
//    typechecker: unifying two type.function values that both carry effects
//    unifies the effects rows.
// ---------------------------------------------------------------------------

#[test]
fn function_with_effects_unifies_effects_field() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let effects_a = type_effects_closed(&["map"]);
    let effects_b = type_effects_closed(&["map"]);
    let fun_a = type_function_with_effects(type_number(), type_string(), effects_a);
    let fun_b = type_function_with_effects(type_number(), type_string(), effects_b);
    // Should succeed: param, body, and effects all match.
    engine.unify(&fun_a, &fun_b, &mut ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 8. A function type with effects vs one without effects: effect unification
//    is skipped (backward compatible).
// ---------------------------------------------------------------------------

#[test]
fn function_with_effects_vs_without_effects_skips_effect_unify() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let effects = type_effects_closed(&["map"]);
    let fun_with = type_function_with_effects(type_number(), type_string(), effects);
    let fun_without = type_function(type_number(), type_string());
    // Should succeed: param and body match; effects skipped (one side absent).
    engine.unify(&fun_with, &fun_without, &mut ctx).unwrap();
    engine.unify(&fun_without, &fun_with, &mut ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 9. A function type with mismatched effects fails to unify.
// ---------------------------------------------------------------------------

#[test]
fn function_with_mismatched_effects_fails() {
    let engine = engine();
    let mut ctx = TypecheckContext::new("handl.canonical", HashMap::new());
    let effects_a = type_effects_closed(&["map"]);
    let effects_b = type_effects_closed(&["serialize"]);
    let fun_a = type_function_with_effects(type_number(), type_string(), effects_a);
    let fun_b = type_function_with_effects(type_number(), type_string(), effects_b);
    let err = engine.unify(&fun_a, &fun_b, &mut ctx).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("cannot unify closed effect rows")),
        "expected closed-effect-row mismatch, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 10. infer_effect_row of a nested Handle scheme (Seq of two handles) unions
//     the targets, producing a type.effects with multiple ops.
// ---------------------------------------------------------------------------

#[test]
fn effect_row_nested_handles_union_ops() {
    let engine = engine();
    let mut lang = Language::new();
    // A type rule whose scheme is a Seq of two Handles. The first handle
    // targets "map", the second targets "serialize". The body of each
    // constructs type.function{param: type.number, body: type.string}.
    // infer_effect_row of the Seq is {map, serialize}. After elimination,
    // the Seq evaluates to the second handle's result (threaded), which is
    // a type.function — so the effects field is attached with both ops.
    //
    // Actually, the Seq threads fst's result into snd. The top-level scheme
    // is a Seq, not a Handle, so infer_effect_row is computed only on Handle
    // nodes. To get the union, we wrap the Seq in a Handle whose body is the
    // Seq. The outer Handle's target adds one more op. Instead, let's use a
    // single Handle whose body is a Seq of two sub-handles.
    //
    // Simpler: a Handle whose body is a Seq(handle(..., "map", ...),
    // handle(..., "serialize", ...)). The outer Handle targets "log".
    // infer_effect_row = {log, map, serialize}.
    let make_fun = Scheme::arr_judgment_lit(
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
    );
    let inner = Scheme::seq(
        Scheme::handle(
            make_fun.clone(),
            "map",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ),
        Scheme::handle(
            make_fun.clone(),
            "serialize",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ),
    );
    lang.types
        .merge(TypeRuleDef::new(
            "type.nested_handler",
            "nhandler",
            Scheme::handle(
                inner,
                "log",
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("nhandler"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The result should be type.function with effects containing
    // {log, map, serialize}.
    assert_eq!(
        result.typ.get("__tag__").and_then(|t| t.as_str()),
        Some("type.function")
    );
    let effects = result
        .typ
        .get("effects")
        .expect("effects field should be present");
    assert_eq!(
        effects.get("__tag__").and_then(|t| t.as_str()),
        Some("type.effects")
    );
    let ops = effects.get("ops").and_then(|o| o.as_array()).unwrap();
    let op_names: Vec<String> = ops
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    assert!(
        op_names.contains(&"map".to_string()),
        "ops should contain 'map': {:?}",
        op_names
    );
    assert!(
        op_names.contains(&"serialize".to_string()),
        "ops should contain 'serialize': {:?}",
        op_names
    );
    assert!(
        op_names.contains(&"log".to_string()),
        "ops should contain 'log': {:?}",
        op_names
    );
    assert_eq!(op_names.len(), 3);
    assert_eq!(effects.get("rest"), Some(&Value::Null));
}

// ===========================================================================
// ADR-0068 step B1: Predicate annotation checking
//
// When a type annotation is a predicate (a function value returning bool),
// the typechecker normalizes `predicate(expr)` via the bound eval engine.
// Proven → accept statically; Disproved → reject; Blocked → accept with
// runtime fallback.
// ===========================================================================

/// Build a minimal language with eval + type rules for predicate annotation
/// checking: `lit`, `var`, `function`, `call`, `binary.mod`, `binary.eq`,
/// and `type-annot`.
fn predicate_lang() -> Language {
    let mut lang = Language::new();

    // --- eval schemes ---
    // lit: no eval scheme → tree-walk returns node.payload directly.
    // var: lookup name from env ($payload = node's payload string).
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.var",
            "var",
            Scheme::arr_native_lit("proj_env", vec![], Value::from("$payload")),
        ))
        .unwrap();
    // function: create a closure.
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.function",
            "function",
            Scheme::arr_native("closure", vec![]),
        ))
        .unwrap();
    // call: apply child 0 (callee) to child 1 (arg).
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.call",
            "call",
            Scheme::app(
                Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                vec![Scheme::arr_native_lit(
                    "proj_child",
                    vec![],
                    Value::from(1u64),
                )],
            ),
        ))
        .unwrap();
    // binary.mod: mod(child(0), child(1))
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.binary.mod",
            "binary.mod",
            Scheme::arr_native(
                "mod",
                vec![
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ],
            ),
        ))
        .unwrap();
    // binary.eq: eq(child(0), child(1))
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.binary.eq",
            "binary.eq",
            Scheme::arr_native(
                "eq",
                vec![
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ],
            ),
        ))
        .unwrap();

    // --- type rules ---
    // lit → type.number
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // var → lookup from Γ
    lang.types
        .merge(TypeRuleDef::new(
            "type.var",
            "var",
            Scheme::arr_judgment_lit("lookup", vec![], Value::from("$payload")),
        ))
        .unwrap();
    // function → synth_function (constructs type.function{param, body})
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::arr_judgment("synth_function", vec![]),
        ))
        .unwrap();
    // binary.mod → type.number
    lang.types
        .merge(TypeRuleDef::new(
            "type.binary.mod",
            "binary.mod",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // binary.eq → type.bool
    lang.types
        .merge(TypeRuleDef::new(
            "type.binary.eq",
            "binary.eq",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
        ))
        .unwrap();
    // type-annot → synth_type_annot
    lang.types
        .merge(TypeRuleDef::new(
            "type.type-annot",
            "type-annot",
            Scheme::arr_judgment("synth_type_annot", vec![]),
        ))
        .unwrap();

    // AST shape roles: function has params/body (ADR-0081).
    lang.ast
        .merge(
            AstShapeDef::new("ast.function", "function").with_children(vec![
                ChildRoleSpec {
                    role: "params".into(),
                    variadic: false,
                },
                ChildRoleSpec {
                    role: "body".into(),
                    variadic: false,
                },
            ]),
        )
        .unwrap();
    lang
}

/// Build an AST for `expr: (n) -> n % 3 == 0` where `expr` is a lit node
/// with value `expr_val`.
///
/// AST layout:
///   0: var("n")                          — parameter
///   1: lit(3)                            — divisor
///   2: binary.mod{children:[0,1]}        — n % 3
///   3: lit(0)                            — zero
///   4: binary.eq{children:[2,3]}         — n % 3 == 0
///   5: function{children:[0,4]}          — (n) -> n % 3 == 0
///   6: lit(expr_val)                     — the expression
///   7: type-annot{children:[6,5]}        — expr: (n) -> n % 3 == 0
fn divisibility_ast(expr_val: i64) -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 1
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5
    ast.push(AstNode::new("lit").with_payload(json!(expr_val))); // 6
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [6, 5]}))); // 7
    ast
}

#[test]
fn predicate_annotation_proven() {
    // `9: (n) -> n % 3 == 0` — 9 % 3 == 0 is true → Proven.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisibility_ast(9);
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // The annotation is accepted; the result type is the expression's type
    // (type.number for a lit).
    assert_eq!(result.typ, type_number());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn predicate_annotation_disproved() {
    // `10: (n) -> n % 3 == 0` — 10 % 3 == 1 ≠ 0 is false → Disproved.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisibility_ast(10);
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("disproved")),
        "expected disproved error, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// ADR-0083: FFI marshalling types, marshalable judgment op, foreign_type
// ---------------------------------------------------------------------------

/// The expected tagged type value for `type.i32` in `handl.canonical`.
fn type_i32() -> Value {
    json!({"__tag__":"type.i32","__registry__":"handl.canonical"})
}

/// The expected tagged type value for `type.cstr` in `handl.canonical`.
fn type_cstr() -> Value {
    json!({"__tag__":"type.cstr","__registry__":"handl.canonical"})
}

#[test]
fn is_marshalable_identifies_ffi_types() {
    let registry = handl_plugin_lib_tag::TypeRegistry::new("handl.canonical");
    assert!(is_marshalable(&registry, &type_i32()));
    assert!(is_marshalable(&registry, &type_cstr()));
    assert!(!is_marshalable(&registry, &type_number()));
    assert!(!is_marshalable(&registry, &type_string()));
}

#[test]
fn marshalable_judgment_accepts_ffi_type() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.i32) >>> marshalable
    lang.types
        .merge(TypeRuleDef::new(
            "type.ffi_arg",
            "ffi_arg",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.i32")),
                Scheme::arr_judgment("marshalable", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("ffi_arg"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, Value::Bool(true));
}

#[test]
fn marshalable_judgment_rejects_non_ffi_type() {
    let engine = engine();
    let mut lang = Language::new();
    // Rule: construct(type.number) >>> marshalable  (should error)
    lang.types
        .merge(TypeRuleDef::new(
            "type.ffi_arg",
            "ffi_arg",
            Scheme::seq(
                Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
                Scheme::arr_judgment("marshalable", vec![]),
            ),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("ffi_arg"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("not FFI-marshalable")),
        "expected not-marshalable error, got {:?}",
        err
    );
}

// --- mock module with a TypeSignature op for foreign_type tests ---

mod ffi_mock {
    use super::*;
    use handl_platform::{EvalContext, EvalError, Function, Module, TypeSignature};

    /// A mock op `ffi.puts` with a declared type signature.
    struct PutsOp;
    impl Function for PutsOp {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(Value::Null)
        }
        fn as_type_signature(&self) -> Option<&dyn TypeSignature> {
            Some(self)
        }
    }
    impl TypeSignature for PutsOp {
        fn type_signature(&self) -> Value {
            json!({"__tag__":"type.function","__registry__":"handl.canonical"})
        }
    }

    /// A mock op with no type signature (plain `Function`).
    struct PlainOp;
    impl Function for PlainOp {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(Value::Null)
        }
    }

    pub struct FfiModule;
    impl Module for FfiModule {
        fn name(&self) -> &str {
            "ffi"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            match name {
                "puts" => Some(&PutsOp),
                "plain" => Some(&PlainOp),
                _ => None,
            }
        }
    }
}

#[test]
fn foreign_type_looks_up_signature_from_module_registry() {
    let registry = ModuleRegistry::new().with(ffi_mock::FfiModule);
    let lookup: Rc<dyn handl_platform::ModuleLookup> = Rc::new(registry);
    let engine = NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()))
        .with_modules(lookup.clone())
        .box_clone();
    let mut lang = Language::new();
    // Rule: foreign_type("ffi.puts")
    lang.types
        .merge(TypeRuleDef::new(
            "type.ffi_call",
            "ffi_call",
            Scheme::arr_judgment_lit("foreign_type", vec![], Value::from("ffi.puts")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("ffi_call"));
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.typ.get("__tag__").and_then(|t| t.as_str()),
        Some("type.function")
    );
}

#[test]
fn foreign_type_errors_when_op_has_no_signature() {
    let registry = ModuleRegistry::new().with(ffi_mock::FfiModule);
    let lookup: Rc<dyn handl_platform::ModuleLookup> = Rc::new(registry);
    let engine = NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()))
        .with_modules(lookup.clone())
        .box_clone();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.ffi_call",
            "ffi_call",
            Scheme::arr_judgment_lit("foreign_type", vec![], Value::from("ffi.plain")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("ffi_call"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("no registered type signature")),
        "expected no-signature error, got {:?}",
        err
    );
}

#[test]

fn predicate_annotation_blocked() {
    // A predicate that errors during evaluation → Blocked → accepted with
    // a `runtime_assert` diagnostic. We use a predicate whose body
    // references an unbound variable (not in the closure's params),
    // causing an eval error.
    //
    // AST:
    //   0: var("n")                          — parameter
    //   1: var("unknown")                    — unbound variable in body
    //   2: function{children:[0,1]}          — (n) -> unknown  (will error)
    //   3: lit(9)                            — expression
    //   4: type-annot{children:[3,2]}        — 9: (n) -> unknown
    let engine = engine();
    let lang = predicate_lang();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("unknown"))); // 1
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("lit").with_payload(json!(9))); // 3
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [3, 2]}))); // 4
    let req = TypecheckRequest::new(ast);
    // Blocked → accepted (not rejected), with a runtime_assert diagnostic.
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    // Exactly one diagnostic: the runtime_assert from the Blocked predicate.
    assert_eq!(result.diagnostics.len(), 1, "expected 1 diagnostic");
    assert_eq!(result.diagnostics[0].severity, "runtime_assert");
    assert!(
        result.diagnostics[0]
            .message
            .contains("runtime assert required"),
        "diagnostic message: {}",
        result.diagnostics[0].message
    );
}

#[test]
fn foreign_type_errors_without_module_registry() {
    let engine = engine();
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new(
            "type.ffi_call",
            "ffi_call",
            Scheme::arr_judgment_lit("foreign_type", vec![], Value::from("ffi.puts")),
        ))
        .unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("ffi_call"));
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("requires a module registry")),
        "expected no-registry error, got {:?}",
        err
    );
}

#[test]

fn predicate_annotation_proven_no_diagnostic() {
    // A proven predicate annotation should produce zero diagnostics —
    // no runtime assert is needed.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisibility_ast(9);
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(
        result.diagnostics.is_empty(),
        "proven annotation should have no diagnostics, got {:?}",
        result.diagnostics
    );
}

#[test]
fn type_signature_trait_works_with_mock() {
    use handl_platform::{EvalContext, EvalError, Function, TypeSignature};
    struct MockOp;
    impl Function for MockOp {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(Value::Null)
        }
        fn as_type_signature(&self) -> Option<&dyn TypeSignature> {
            Some(self)
        }
    }
    impl TypeSignature for MockOp {
        fn type_signature(&self) -> Value {
            json!({"__tag__":"type.function","__registry__":"handl.canonical"})
        }
    }
    let op = MockOp;
    let func: &dyn Function = &op;
    let ts = func
        .as_type_signature()
        .expect("should expose TypeSignature");
    assert_eq!(
        ts.type_signature().get("__tag__").and_then(|t| t.as_str()),
        Some("type.function")
    );
}

#[test]

fn non_predicate_annotation_regression() {
    // `42: type.number` — a non-predicate annotation should still work via
    // the existing unify path. The type expression is `field-access(var("type"),
    // "number")` which `interpret_type_expr` handles.
    let engine = engine();
    let mut lang = predicate_lang();
    // var type rule for "type" variable — we need it in Γ. Use extend.
    // Actually, for `type.number`, the type expression is a field-access
    // node. We need a `field-access` type rule that interpret_type_expr
    // handles. But interpret_type_expr is called directly, not via a type
    // rule. So we just need the type-annot rule (already in predicate_lang)
    // and the field-access node to be interpretable.
    // The field-access node has children [var("type"), var("number")].
    // interpret_type_expr reads the field name from child 1's payload.
    // We need var nodes with string payloads.
    // No additional type rules needed — interpret_type_expr bypasses
    // synth_node for the type expression.
    let _ = &mut lang; // suppress unused mut warning

    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("type"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("number"))); // 1
    ast.push(AstNode::new("field-access").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 3
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [3, 2]}))); // 4
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
}

// ===========================================================================
// ADR-0068 step B2: Symbolic normalization for predicates with free variables
//
// B2 fires when B1 (concrete eval) returns Blocked. The symbolic normalizer
// works on the AST directly, handling free variables and compound expressions
// that the eval engine can't evaluate.
// ===========================================================================

#[test]
fn b2_symbolic_reflexivity_proven() {
    // Predicate: (n) -> n == n
    // Expression: var("x") (a free variable)
    //
    // B1: eval engine tries to evaluate `predicate(x)` — `x` is unbound →
    //     error → Blocked.
    // B2: symbolic normalizer substitutes n = x, then normalizes
    //     `Eq(Var("x"), Var("x"))` → `Bool(true)` → Proven!
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 0]}))); // 1: n == n
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 1]}))); // 2: (n) -> n == n
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 3: var("x")

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(2),
        AstRef::new(3),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_symbolic_arithmetic_normalization() {
    // Predicate: (n) -> n == n + 0
    // Expression: var("x")
    //
    // B2: normalizes x + 0 → x, then x == x → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1: lit(0)
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]}))); // 2: n + 0
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 2]}))); // 3: n == n + 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 3]}))); // 4: (n) -> n == n + 0
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 5

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(4),
        AstRef::new(5),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_symbolic_concrete_disproved() {
    // Predicate: (n) -> n % 3 == 0
    // Expression: lit(10) — 10 % 3 == 1 ≠ 0 → false → Disproved.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 1: lit(3)
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2: n % 3
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3: lit(0)
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4: n % 3 == 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5: (n) -> n % 3 == 0
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 6: lit(10)

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(5),
        AstRef::new(6),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Disproved);
}

#[test]
fn b2_symbolic_concrete_proven() {
    // Predicate: (n) -> n % 3 == 0
    // Expression: lit(9) — 9 % 3 == 0 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 1: lit(3)
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2: n % 3
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3: lit(0)
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4: n % 3 == 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5: (n) -> n % 3 == 0
    ast.push(AstNode::new("lit").with_payload(json!(9))); // 6: lit(9)

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(5),
        AstRef::new(6),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_symbolic_free_variable_blocked() {
    // Predicate: (n) -> n % 3 == 0
    // Expression: var("x") — free variable
    //
    // B2: x % 3 can't reduce → stuck → Blocked (correct: we can't prove
    //     x % 3 == 0 without knowing x).
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 1: lit(3)
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2: n % 3
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3: lit(0)
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4: n % 3 == 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5: (n) -> n % 3 == 0
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 6: var("x")

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(5),
        AstRef::new(6),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Blocked);
}

#[test]
fn b2_symbolic_unsupported_op_blocked() {
    // Predicate body uses an unsupported operation (field-access).
    // B2 should return Blocked.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("lit").with_payload(json!("field"))); // 1
    ast.push(AstNode::new("field-access").with_payload(json!({"children": [0, 1]}))); // 2: n.field
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3: (n) -> n.field
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Blocked);
}

#[test]
fn module_registry_lookup_type_signature_returns_signature() {
    let registry = ModuleRegistry::new().with(ffi_mock::FfiModule);
    let sig = registry.lookup_type_signature("ffi.puts");
    assert!(sig.is_some());
    assert_eq!(
        sig.unwrap().get("__tag__").and_then(|t| t.as_str()),
        Some("type.function")
    );
    // Plain op has no signature.
    assert!(registry.lookup_type_signature("ffi.plain").is_none());
    // Unknown op.
    assert!(registry.lookup_type_signature("ffi.missing").is_none());
}

// ===========================================================================
// Phase 3.1: B2 handles named/parametric predicates (call nodes)
// ===========================================================================

/// Build an AST for `DivisibleBy(3)` where `DivisibleBy` is an inline
/// `(k) -> (n) -> n % k == 0` and the expression is `expr_val`.
///
/// AST layout:
///   0: var("k")                          — outer param
///   1: var("n")                          — inner param
///   2: binary.mod{children:[1,0]}        — n % k
///   3: lit(0)                            — zero
///   4: binary.eq{children:[2,3]}         — n % k == 0
///   5: function{children:[1,4]}          — (n) -> n % k == 0  (inner)
///   6: function{children:[0,5]}          — (k) -> (n) -> ...  (outer)
///   7: lit(3)                            — argument to outer
///   8: call{children:[6,7]}             — DivisibleBy(3) → closure
///   9: <expr_node>                       — the annotated expression
fn divisible_by_call_ast(expr_val: i64) -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("k"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 1
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [1, 0]}))); // 2
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 4]}))); // 5
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 5]}))); // 6
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 7
    ast.push(AstNode::new("call").with_payload(json!({"children": [6, 7]}))); // 8
    ast.push(AstNode::new("lit").with_payload(json!(expr_val))); // 9
    ast
}

#[test]
fn b2_parametric_predicate_call_proven() {
    // DivisibleBy(3) defined as (k) -> (n) -> n % k == 0
    // Expression: lit(6) — 6 % 3 == 0 → true → Proven.
    //
    // B2 evaluates the call node to get the closure, then normalizes
    // 6 % 3 == 0 → 0 == 0 → true.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisible_by_call_ast(6);
    let proof =
        engine.check_predicate_symbolic(&lang, &ast, AstRef::new(8), AstRef::new(9), 100_000);
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_parametric_predicate_call_disproved() {
    // DivisibleBy(3) with expression lit(7) — 7 % 3 == 1 ≠ 0 → false.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisible_by_call_ast(7);
    let proof =
        engine.check_predicate_symbolic(&lang, &ast, AstRef::new(8), AstRef::new(9), 100_000);
    assert_eq!(proof.verdict, Verdict::Disproved);
}

#[test]
fn b2_parametric_predicate_call_free_var_blocked() {
    // DivisibleBy(3) with expression var("y") — free variable.
    // B2 can't prove y % 3 == 0 → Blocked (not an error).
    let engine = engine();
    let lang = predicate_lang();
    let mut ast = divisible_by_call_ast(0);
    // Replace the expression (node 9) with var("y").
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 10
    let proof =
        engine.check_predicate_symbolic(&lang, &ast, AstRef::new(8), AstRef::new(10), 100_000);
    assert_eq!(proof.verdict, Verdict::Blocked);
}

// ===========================================================================
// Phase 3.2: B2 handles params/body payload shape
// ===========================================================================

#[test]
fn b2_params_body_payload_shape() {
    // A function node with the rich {params: ["n"], body: idx} payload
    // shape (instead of the legacy {children: [param_idx, body_idx]}).
    // Predicate: (n) -> n % 3 == 0, expression: lit(9) → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 1
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4
    // Rich payload: params + body (not children array)
    ast.push(AstNode::new("function").with_payload(json!({"params": ["n"], "body": 4}))); // 5
    ast.push(AstNode::new("lit").with_payload(json!(9))); // 6

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(5),
        AstRef::new(6),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

// ===========================================================================
// Phase 3.3: B2 supports neq/lt/gt/lte/gte/if/call
// ===========================================================================

#[test]
fn b2_neq_proven() {
    // (n) -> n != 0, expression: lit(5) → 5 != 0 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.neq").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(5))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_neq_disproved() {
    // (n) -> n != 0, expression: lit(0) → 0 != 0 → false → Disproved.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.neq").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Disproved);
}

#[test]
fn b2_lt_proven() {
    // (n) -> n < 10, expression: lit(5) → 5 < 10 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 1
    ast.push(AstNode::new("binary.lt").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(5))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_gt_proven() {
    // (n) -> n > 0, expression: lit(5) → 5 > 0 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(5))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_lte_proven() {
    // (n) -> n <= 10, expression: lit(10) → 10 <= 10 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 1
    ast.push(AstNode::new("binary.lte").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_gte_proven() {
    // (n) -> n >= 10, expression: lit(10) → 10 >= 10 → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 1
    ast.push(AstNode::new("binary.gte").with_payload(json!({"children": [0, 1]}))); // 2
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(10))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_if_proven() {
    // (n) -> if n == 0 then true else false
    // expression: lit(0) → if true then true else false → true → Proven.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]}))); // 2: n == 0
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 3: true
    ast.push(AstNode::new("lit").with_payload(json!(false))); // 4: false
    ast.push(AstNode::new("if").with_payload(json!({"children": [2, 3, 4]}))); // 5
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 5]}))); // 6
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 7: expr = 0

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(6),
        AstRef::new(7),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn b2_if_disproved() {
    // (n) -> if n == 0 then true else false
    // expression: lit(5) → if false then true else false → false → Disproved.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]}))); // 2: n == 0
    ast.push(AstNode::new("lit").with_payload(json!(true))); // 3: true
    ast.push(AstNode::new("lit").with_payload(json!(false))); // 4: false
    ast.push(AstNode::new("if").with_payload(json!({"children": [2, 3, 4]}))); // 5
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 5]}))); // 6
    ast.push(AstNode::new("lit").with_payload(json!(5))); // 7: expr = 5

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(6),
        AstRef::new(7),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Disproved);
}

#[test]
fn b2_call_blocked() {
    // (n) -> f(n) where f is a free variable — call can't beta-reduce → Stuck.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 1
    ast.push(AstNode::new("call").with_payload(json!({"children": [1, 0]}))); // 2: f(n)
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 4

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(3),
        AstRef::new(4),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Blocked);
}

// ===========================================================================
// Phase 3.4: Stuck propagates through operations with reason messages
// ===========================================================================

#[test]
fn b2_stuck_propagates_through_add() {
    // (n) -> n + 1 == 1, expression: var("x") (free variable)
    //
    // B2: x is unbound → Var("x"). n + 1 → Add(Var("x"), Int(1)) → can't
    // reduce. The Stuck from the var lookup should propagate through Add
    // into Eq, so the Blocked message contains the stuck reason, not just
    // "Add(...)" wrapper description.
    //
    // Actually, var("x") is not Stuck — it's Var("x") (a free variable).
    // The Add(Var("x"), Int(1)) can't reduce because neither side is Int.
    // The Eq(Add(Var, Int), Int(1)) can't reduce either. So the result is
    // Eq(Add(Var("x"), Int(1)), Int(1)) which doesn't normalize to bool.
    // This maps to Blocked via the catch-all.
    //
    // To test stuck propagation specifically, we use an unsupported op
    // inside an Add: field-access produces Stuck, which should propagate.
    let engine = engine();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!("field"))); // 1
    ast.push(AstNode::new("field-access").with_payload(json!({"children": [0, 1]}))); // 2: n.field → Stuck
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 3
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [2, 3]}))); // 4: n.field + 1
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 5
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [4, 5]}))); // 6: n.field + 1 == 1
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 6]}))); // 7
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 8

    let proof = engine.check_predicate_symbolic(
        &Language::new(),
        &ast,
        AstRef::new(7),
        AstRef::new(8),
        100_000,
    );
    assert_eq!(proof.verdict, Verdict::Blocked);
    // The stuck reason from the field-access should propagate through Add
    // and Eq, so the message contains "unsupported node kind" (the original
    // stuck reason), not just "Add(...)" or "Eq(...)".
    assert!(
        proof.reason.contains("unsupported node kind"),
        "expected stuck reason to propagate, got: {}",
        proof.reason
    );
}

// ===========================================================================
// Phase 3.6: B1→B2 fallback end-to-end tests via synth_type_annot
// ===========================================================================

#[test]
fn b1_b2_fallback_b1_proven_no_b2() {
    // B1 Proven → no B2 call (no runtime_assert diagnostic).
    // Predicate: (n) -> n % 3 == 0, expression: lit(9) → 9 % 3 == 0 → true.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisibility_ast(9);
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(
        result.diagnostics.is_empty(),
        "proven annotation should have no diagnostics, got {:?}",
        result.diagnostics
    );
}

#[test]
fn b1_b2_fallback_b1_blocked_b2_proven() {
    // B1 Blocked, B2 Proven → result is Proven (B2 rescued).
    //
    // Predicate: (n) -> n != 0, expression: lit(5).
    // The eval engine has no scheme for `binary.neq`, so B1 fails (eval
    // error → Blocked). B2 supports `neq` and normalizes
    // `Neq(Int(5), Int(0))` → `Bool(true)` → Proven.
    //
    // We need a type rule for `binary.neq` (so the predicate typechecks)
    // but NO eval scheme (so B1 can't evaluate it).
    let engine = engine();
    let mut lang = predicate_lang();
    // Add type rule for binary.neq → type.bool (but no eval scheme!).
    lang.types
        .merge(TypeRuleDef::new(
            "type.binary.neq",
            "binary.neq",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.bool")),
        ))
        .unwrap();

    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
    ast.push(AstNode::new("binary.neq").with_payload(json!({"children": [0, 1]}))); // 2: n != 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("lit").with_payload(json!(5))); // 4: expr = 5
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [4, 3]}))); // 5

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // B2 rescued → Proven → no runtime_assert diagnostic.
    assert!(
        result.diagnostics.is_empty(),
        "B2-proven annotation should have no diagnostics, got {:?}",
        result.diagnostics
    );
}

#[test]
fn b1_b2_fallback_b1_blocked_b2_blocked() {
    // B1 Blocked, B2 Blocked → result is Blocked (runtime_assert diagnostic).
    //
    // Predicate: (n) -> n % unknown == 0, expression: lit(9).
    // The body references `unknown` which is not bound anywhere.
    // B1: eval fails (unbound `unknown`) → Blocked.
    // B2: `Mod(Int(9), Var("unknown"))` → can't reduce → Stuck → Blocked.
    let engine = engine();
    let lang = predicate_lang();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("unknown"))); // 1: unbound
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2: n % unknown
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 3
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5
    ast.push(AstNode::new("lit").with_payload(json!(9))); // 6: expr = 9
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [6, 5]}))); // 7

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // Both B1 and B2 Blocked → runtime_assert diagnostic.
    assert_eq!(
        result.diagnostics.len(),
        1,
        "expected 1 runtime_assert diagnostic, got {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].severity, "runtime_assert");
}

#[test]
fn b1_b2_fallback_b1_disproved_no_b2() {
    // B1 Disproved → no B2 call (B1 already decided).
    // Predicate: (n) -> n % 3 == 0, expression: lit(10) → 10 % 3 == 1 ≠ 0.
    let engine = engine();
    let lang = predicate_lang();
    let ast = divisibility_ast(10);
    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("disproved")),
        "expected disproved error, got {:?}",
        err
    );
}

// ===========================================================================
// Phase 3.6: B1→B2 fallback e2e with parametric predicate (DivisibleBy(3))
// ===========================================================================

#[test]
fn b1_b2_fallback_parametric_predicate_proven() {
    // DivisibleBy(3) defined as (k) -> (n) -> n % k == 0
    // Expression: lit(6) → 6 % 3 == 0 → true → Proven (via B1).
    //
    // The type expression is a `call` node. interpret_type_expr fails →
    // eval engine evaluates it → closure → B1 applies it → Proven.
    let engine = engine();
    let lang = predicate_lang();
    let mut ast = divisible_by_call_ast(6);
    // Append type-annot{children: [expr_idx, type_idx]}
    // expr = node 9 (lit(6)), type = node 8 (call DivisibleBy(3))
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [9, 8]}))); // 10
    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(result.typ, type_number());
    assert!(
        result.diagnostics.is_empty(),
        "proven parametric predicate should have no diagnostics, got {:?}",
        result.diagnostics
    );
}

#[test]
fn b1_b2_fallback_parametric_predicate_blocked() {
    // DivisibleBy(3) with a predicate body that references an unbound var
    // in a position that causes a B1 eval error.
    // The outer function is (k) -> (n) -> unknown_var % k == 0.
    // B1: `unknown_var` resolves to the string "unknown_var" (proj_env
    //     returns the name for unbound vars). `mod("unknown_var", 3)`
    //     errors (mod expects ints) → Blocked.
    // B2: `Mod(Var("unknown_var"), Int(3))` → can't reduce → Stuck → Blocked.
    // Overall: Blocked (runtime_assert diagnostic), not an error.
    let engine = engine();
    let lang = predicate_lang();
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("k"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 1
    ast.push(AstNode::new("var").with_payload(json!("unknown_var"))); // 2: unbound
    ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [2, 0]}))); // 3: unknown_var % k
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 4
    ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [3, 4]}))); // 5: unknown_var % k == 0
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 5]}))); // 6: (n) -> ...
    ast.push(AstNode::new("function").with_payload(json!({"children": [0, 6]}))); // 7: (k) -> ...
    ast.push(AstNode::new("lit").with_payload(json!(3))); // 8
    ast.push(AstNode::new("call").with_payload(json!({"children": [7, 8]}))); // 9: DivisibleBy(3)
    ast.push(AstNode::new("lit").with_payload(json!(6))); // 10: expr = 6
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [10, 9]}))); // 11

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert_eq!(
        result.diagnostics.len(),
        1,
        "expected 1 runtime_assert diagnostic, got {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].severity, "runtime_assert");
}

// ===========================================================================
// Phase 3.7: B1/B2 agreement test on concrete cases
// ===========================================================================

#[test]
fn b1_b2_agreement_concrete_cases() {
    // For predicates where both B1 and B2 can run (concrete expressions,
    // no free vars), assert they agree: both Proven, both Disproved, or
    // both Blocked — never one Proven and the other Disproved.
    let engine = engine();
    let lang = predicate_lang();

    // Test with various concrete values against (n) -> n % 3 == 0.
    for &val in &[0i64, 1, 2, 3, 6, 9, 10, 12, 15, 21, 100, -3, -6] {
        let ast = divisibility_ast(val);
        let b1_proof = engine.check_predicate_application(
            &lang,
            &ast,
            AstRef::new(5), // function node
            AstRef::new(6), // expr (lit)
            100_000,
        );
        let b2_proof =
            engine.check_predicate_symbolic(&lang, &ast, AstRef::new(5), AstRef::new(6), 100_000);
        // They must agree (never one Proven and the other Disproved).
        assert_ne!(
            (
                b1_proof.verdict == Verdict::Proven,
                b2_proof.verdict == Verdict::Proven
            ),
            (true, false),
            "B1/B2 disagreement at val={}: B1={:?}, B2={:?}",
            val,
            b1_proof.verdict,
            b2_proof.verdict
        );
        assert_ne!(
            (
                b1_proof.verdict == Verdict::Proven,
                b2_proof.verdict == Verdict::Proven
            ),
            (false, true),
            "B1/B2 disagreement at val={}: B1={:?}, B2={:?}",
            val,
            b1_proof.verdict,
            b2_proof.verdict
        );
        assert_ne!(
            (
                b1_proof.verdict == Verdict::Disproved,
                b2_proof.verdict == Verdict::Disproved
            ),
            (true, false),
            "B1/B2 disagreement at val={}: B1={:?}, B2={:?}",
            val,
            b1_proof.verdict,
            b2_proof.verdict
        );
        assert_ne!(
            (
                b1_proof.verdict == Verdict::Disproved,
                b2_proof.verdict == Verdict::Disproved
            ),
            (false, true),
            "B1/B2 disagreement at val={}: B1={:?}, B2={:?}",
            val,
            b1_proof.verdict,
            b2_proof.verdict
        );
        // Both should give a definitive verdict for concrete values.
        assert!(
            b1_proof.verdict != Verdict::Blocked && b2_proof.verdict != Verdict::Blocked,
            "B1/B2 should not block on concrete value {}: B1={:?}, B2={:?}",
            val,
            b1_proof.verdict,
            b2_proof.verdict
        );
    }
}

#[test]
fn b1_b2_agreement_property_test() {
    // Property test: generate random concrete int expressions, run both
    // B1 and B2 on (n) -> n % k == 0 style predicates, assert agreement.
    use std::time::SystemTime;
    let engine = engine();
    let lang = predicate_lang();

    // Simple LCG for deterministic pseudo-random values.
    let mut state = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let mut next_rand = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state >> 33
    };

    for _ in 0..200 {
        let k = (next_rand() % 10) as i64 + 1; // 1..=10
        let n = (next_rand() % 100) as i64; // 0..=99

        // Build AST for (n) -> n % k == 0 with expression lit(n).
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
        ast.push(AstNode::new("lit").with_payload(json!(k))); // 1
        ast.push(AstNode::new("binary.mod").with_payload(json!({"children": [0, 1]}))); // 2
        ast.push(AstNode::new("lit").with_payload(json!(0))); // 3
        ast.push(AstNode::new("binary.eq").with_payload(json!({"children": [2, 3]}))); // 4
        ast.push(AstNode::new("function").with_payload(json!({"children": [0, 4]}))); // 5
        ast.push(AstNode::new("lit").with_payload(json!(n))); // 6

        let b1_proof = engine.check_predicate_application(
            &lang,
            &ast,
            AstRef::new(5),
            AstRef::new(6),
            100_000,
        );
        let b2_proof =
            engine.check_predicate_symbolic(&lang, &ast, AstRef::new(5), AstRef::new(6), 100_000);

        // Compute expected result independently.
        let expected = if k != 0 { n % k == 0 } else { false };

        match (expected, &b1_proof.verdict, &b2_proof.verdict) {
            (true, Verdict::Proven, Verdict::Proven) => {}
            (false, Verdict::Disproved, Verdict::Disproved) => {}
            other => panic!(
                "B1/B2 disagreement at n={}, k={}: expected={}, B1={:?}, B2={:?}",
                n, k, expected, other.1, other.2
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// ADR-0082 step 3: Termination metadata reading
// ---------------------------------------------------------------------------

#[test]
fn read_termination_metadata_from_function_node() {
    // A function node with a `termination` payload field should be
    // recognized as structurally recursive.
    let node = AstNode::new("function").with_payload(json!({
        "params": ["n"],
        "body": 0,
        "termination": {
            "decreasing_arg": 0,
            "decreasing_field": "pred"
        }
    }));
    let info = read_termination_metadata(&node).expect("termination metadata");
    assert_eq!(info.decreasing_arg, 0);
    assert_eq!(info.decreasing_field, "pred");
}

#[test]
fn read_termination_metadata_absent_returns_none() {
    // A function node without a `termination` field should return None.
    let node = AstNode::new("function").with_payload(json!({
        "params": ["x"],
        "body": 0
    }));
    assert!(read_termination_metadata(&node).is_none());
}

#[test]
fn read_termination_metadata_non_function_node() {
    // A non-function node with a `termination` field should still be
    // readable — the helper is payload-driven, not kind-driven.
    let node = AstNode::new("assign").with_payload(json!({
        "children": [0, 1],
        "termination": {
            "decreasing_arg": 2,
            "decreasing_field": "left"
        }
    }));
    let info = read_termination_metadata(&node).expect("termination metadata");
    assert_eq!(info.decreasing_arg, 2);
    assert_eq!(info.decreasing_field, "left");
}

// ===========================================================================
// Phase 7: Total and Pure meta-predicates
//
// Total(A -> B) asserts a function is total (terminates) via structural
// recursion checking. Pure(A -> B) asserts a function is pure (effect-free)
// by checking its effect row is empty. Both are meta-predicates handled
// specially in synth_type_annot.
// ===========================================================================

/// Build a minimal language for meta-predicate integration tests: `lit`,
/// `var`, `function`, `binary.add`, and `type-annot` type rules, plus eval
/// schemes for `var` and `function` (needed by the bound eval engine when
/// the predicate path falls back to it).
fn meta_predicate_lang() -> Language {
    let mut lang = Language::new();

    // --- eval schemes (for closure creation / var lookup) ---
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.var",
            "var",
            Scheme::arr_native_lit("proj_env", vec![], Value::from("$payload")),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "eval.function",
            "function",
            Scheme::arr_native("closure", vec![]),
        ))
        .unwrap();

    // --- type rules ---
    // lit → type.number
    lang.types
        .merge(TypeRuleDef::new(
            "type.lit",
            "lit",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // var → lookup from Γ
    lang.types
        .merge(TypeRuleDef::new(
            "type.var",
            "var",
            Scheme::arr_judgment_lit("lookup", vec![], Value::from("$payload")),
        ))
        .unwrap();
    // function → synth_function
    lang.types
        .merge(TypeRuleDef::new(
            "type.function",
            "function",
            Scheme::arr_judgment("synth_function", vec![]),
        ))
        .unwrap();
    // binary.add → type.number (both children type.number)
    lang.types
        .merge(TypeRuleDef::new(
            "type.binary.add",
            "binary.add",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.number")),
        ))
        .unwrap();
    // type-annot → synth_type_annot
    lang.types
        .merge(TypeRuleDef::new(
            "type.type-annot",
            "type-annot",
            Scheme::arr_judgment("synth_type_annot", vec![]),
        ))
        .unwrap();
    // assign → type.unit (statement with side effects; matches canonical)
    lang.types
        .merge(TypeRuleDef::new(
            "type.assign",
            "assign",
            Scheme::arr_judgment_lit("construct", vec![], Value::from("type.unit")),
        ))
        .unwrap();

    lang.ast
        .merge(
            AstShapeDef::new("ast.paren", "paren").with_children(vec![ChildRoleSpec {
                role: "body".into(),
                variadic: true,
            }]),
        )
        .unwrap();
    lang
}

// --- 7.1: Total predicate — check_totality unit tests ---

#[test]
fn total_nonrecursive_function_is_trivially_total() {
    // `(x) -> x + 1` — anonymous, non-recursive → trivially total.
    // AST:
    //   0: var("x")                       — param
    //   1: paren{children:[0]}            — params wrapper
    //   2: lit(1)                         — 1
    //   3: binary.add{children:[0,2]}     — x + 1
    //   4: function{children:[1,3]}       — (x) -> x + 1
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 2
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 3]}))); // 4

    let lang = Language::new();
    let proof = totality_check::check_totality(&ast, AstRef::new(4), &lang);
    assert_eq!(proof.verdict, Verdict::Proven);
    assert!(
        proof.reason.contains("trivially total"),
        "expected trivially total, got: {}",
        proof.reason
    );
}

#[test]
fn total_recursor_with_termination_metadata_is_total() {
    // A generated recursor function carrying `termination` metadata →
    // recognized as structurally recursive → Proven.
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1: params
    ast.push(AstNode::new("lit").with_payload(json!(0))); // 2: body (placeholder)
    ast.push(AstNode::new("function").with_payload(json!({
        "children": [1, 2],
        "termination": {
            "decreasing_arg": 0,
            "decreasing_field": "pred"
        }
    }))); // 3: recursor function

    let lang = Language::new();
    let proof = totality_check::check_totality(&ast, AstRef::new(3), &lang);
    assert_eq!(proof.verdict, Verdict::Proven);
    assert!(
        proof.reason.contains("recursor metadata"),
        "expected recursor metadata reason, got: {}",
        proof.reason
    );
}

#[test]
fn total_recursive_function_without_decreasing_is_disproved() {
    // A named recursive function `f = (n) -> f(n)` — the recursive call
    // passes `n` directly (not a strict sub-structure) → Disproved.
    //
    // AST:
    //   0: var("n")                        — param
    //   1: paren{children:[0]}             — params wrapper
    //   2: var("f")                        — callee (recursive call)
    //   3: var("n")                        — arg (bare param, not decreasing)
    //   4: call{children:[2,3]}            — f(n)
    //   5: function{children:[1,4]}        — (n) -> f(n)
    //   6: var("f")                        — assign target
    //   7: assign{children:[6,5]}          — f = (n) -> f(n)
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 2
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 3
    ast.push(AstNode::new("call").with_payload(json!({"children": [2, 3]}))); // 4
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 4]}))); // 5
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 6
    ast.push(AstNode::new("assign").with_payload(json!({"children": [6, 5]}))); // 7

    let lang = Language::new();
    // Check totality of the assign node (named function definition).
    let proof = totality_check::check_totality(&ast, AstRef::new(7), &lang);
    assert_eq!(proof.verdict, Verdict::Disproved);
    assert!(
        proof.reason.contains("does not decrease"),
        "expected does-not-decrease reason, got: {}",
        proof.reason
    );
}

#[test]
fn total_recursive_function_with_field_access_decreasing_is_proven() {
    // A named recursive function `f = (n) -> f(n.pred)` — the recursive
    // call passes `n.pred` (a field-access on the param) → strict
    // sub-structure → Proven.
    //
    // AST:
    //   0: var("n")                        — param
    //   1: paren{children:[0]}             — params wrapper
    //   2: var("f")                        — callee
    //   3: var("n")                        — field-access target
    //   4: var("pred")                     — field name
    //   5: field-access{children:[3,4]}    — n.pred
    //   6: call{children:[2,5]}            — f(n.pred)
    //   7: function{children:[1,6]}        — (n) -> f(n.pred)
    //   8: var("f")                        — assign target
    //   9: assign{children:[8,7]}          — f = (n) -> f(n.pred)
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 2
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 3
    ast.push(AstNode::new("var").with_payload(json!("pred"))); // 4
    ast.push(AstNode::new("field-access").with_payload(json!({"children": [3, 4]}))); // 5
    ast.push(AstNode::new("call").with_payload(json!({"children": [2, 5]}))); // 6
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 6]}))); // 7
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 8
    ast.push(AstNode::new("assign").with_payload(json!({"children": [8, 7]}))); // 9

    let lang = Language::new();
    let proof = totality_check::check_totality(&ast, AstRef::new(9), &lang);
    assert_eq!(proof.verdict, Verdict::Proven);
    assert!(
        proof.reason.contains("structurally recursive"),
        "expected structurally recursive reason, got: {}",
        proof.reason
    );
}

#[test]
fn total_var_reference_looks_up_assign() {
    // `f` referenced as a var — totality checker looks up the assign for
    // `f` and checks it. The function is non-recursive → trivially total.
    //
    // AST:
    //   0: var("x")                        — param
    //   1: paren{children:[0]}             — params wrapper
    //   2: lit(1)                          — 1
    //   3: binary.add{children:[0,2]}      — x + 1
    //   4: function{children:[1,3]}        — (x) -> x + 1
    //   5: var("f")                        — assign target
    //   6: assign{children:[5,4]}          — f = (x) -> x + 1
    //   7: var("f")                        — reference to check
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 2
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 3]}))); // 4
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 5
    ast.push(AstNode::new("assign").with_payload(json!({"children": [5, 4]}))); // 6
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 7

    let lang = Language::new();
    let proof = totality_check::check_totality(&ast, AstRef::new(7), &lang);
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn total_non_function_expression_is_blocked() {
    // A lit node is not a function → Blocked.
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));

    let lang = Language::new();
    let proof = totality_check::check_totality(&ast, AstRef::new(0), &lang);
    assert_eq!(proof.verdict, Verdict::Blocked);
}

// --- 7.2: Pure predicate — check_purity unit tests ---

#[test]
fn pure_function_with_no_effects_field_is_pure() {
    // A function type with no `effects` field → pure by default.
    let engine = engine();
    let fn_type = json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": type_number(),
        "body": type_number()
    });
    let proof = engine.check_purity(&fn_type);
    assert_eq!(proof.verdict, Verdict::Proven);
    assert!(
        proof.reason.contains("no effects"),
        "expected no-effects reason, got: {}",
        proof.reason
    );
}

#[test]
fn pure_function_with_empty_effects_is_pure() {
    // A function type with an empty effects ops array → pure.
    let engine = engine();
    let fn_type = json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": type_number(),
        "body": type_number(),
        "effects": {
            "__tag__": "type.effects",
            "__registry__": "handl.canonical",
            "ops": []
        }
    });
    let proof = engine.check_purity(&fn_type);
    assert_eq!(proof.verdict, Verdict::Proven);
}

#[test]
fn pure_function_with_effects_is_disproved() {
    // A function type with a non-empty effects ops array → impure.
    let engine = engine();
    let fn_type = json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": type_number(),
        "body": type_number(),
        "effects": {
            "__tag__": "type.effects",
            "__registry__": "handl.canonical",
            "ops": ["Foreign"]
        }
    });
    let proof = engine.check_purity(&fn_type);
    assert_eq!(proof.verdict, Verdict::Disproved);
    assert!(
        proof.reason.contains("Foreign"),
        "expected effect name in reason, got: {}",
        proof.reason
    );
}

#[test]
fn pure_non_function_type_is_blocked() {
    // A non-function type → cannot check purity → Blocked.
    let engine = engine();
    let proof = engine.check_purity(&type_number());
    assert_eq!(proof.verdict, Verdict::Blocked);
}

// --- Integration: Total / Pure through synth_type_annot ---

/// Build a `type.number` field-access type-expression subtree, returning
/// the index of the root `field-access` node. Pushes 3 nodes: var("type"),
/// var("number"), field-access{children:[type_idx, number_idx]}.
fn push_type_number_expr(ast: &mut Ast, type_idx_offset: usize) -> usize {
    let type_idx = type_idx_offset;
    let number_idx = type_idx + 1;
    let fa_idx = type_idx + 2;
    ast.push(AstNode::new("var").with_payload(json!("type"))); // type_idx
    ast.push(AstNode::new("var").with_payload(json!("number"))); // number_idx
    ast.push(
        AstNode::new("field-access").with_payload(json!({"children": [type_idx, number_idx]})),
    ); // fa_idx
    fa_idx
}

#[test]
fn total_annotation_on_nonrecursive_function_proven() {
    // `(x) -> x + 1 : Total(type.number -> type.number)` → Proven.
    //
    // The type expression is `call{var("Total"), function{...}}`.
    let engine = engine();
    let lang = meta_predicate_lang();

    let mut ast = Ast::new();
    // --- expression: (x) -> x + 1 ---
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: param
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1: params
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 2: 1
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 2]}))); // 3: x+1
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 3]}))); // 4: fn

    // --- type expression: Total(type.number -> type.number) ---
    let param_fa = push_type_number_expr(&mut ast, 5); // 5,6,7 → 7
    let body_fa = push_type_number_expr(&mut ast, 8); // 8,9,10 → 10
    ast.push(AstNode::new("function").with_payload(json!({"children": [param_fa, body_fa]}))); // 11: type.number -> type.number
    ast.push(AstNode::new("var").with_payload(json!("Total"))); // 12: callee
    ast.push(AstNode::new("call").with_payload(json!({"children": [12, 11]}))); // 13: Total(...)

    // --- annotation ---
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [4, 13]}))); // 14

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // Proven → annotation accepted, no diagnostics.
    assert!(
        result.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        result.diagnostics
    );
    // The accepted type is the annotation type (type.Total).
    let tag = result.typ.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(tag, Some("type.Total"));
}

#[test]
fn pure_annotation_on_pure_function_proven() {
    // `(x) -> x + 1 : Pure(type.number -> type.number)` → Proven.
    // synth_function produces a type.function with no `effects` field → pure.
    let engine = engine();
    let lang = meta_predicate_lang();

    let mut ast = Ast::new();
    // --- expression: (x) -> x + 1 ---
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 2
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 2]}))); // 3
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 3]}))); // 4

    // --- type expression: Pure(type.number -> type.number) ---
    let param_fa = push_type_number_expr(&mut ast, 5); // 7
    let body_fa = push_type_number_expr(&mut ast, 8); // 10
    ast.push(AstNode::new("function").with_payload(json!({"children": [param_fa, body_fa]}))); // 11
    ast.push(AstNode::new("var").with_payload(json!("Pure"))); // 12
    ast.push(AstNode::new("call").with_payload(json!({"children": [12, 11]}))); // 13

    // --- annotation ---
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [4, 13]}))); // 14

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    assert!(
        result.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        result.diagnostics
    );
    let tag = result.typ.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(tag, Some("type.Pure"));
}

#[test]
fn total_annotation_on_non_decreasing_recursion_disproved() {
    // `f = (n) -> f(n) : Total(...)` via assign — the recursive call does
    // not decrease → Disproved → typecheck errors.
    //
    // We annotate the `assign` node itself with Total.
    let engine = engine();
    let lang = meta_predicate_lang();

    let mut ast = Ast::new();
    // --- expression: f = (n) -> f(n) ---
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: param
    ast.push(AstNode::new("paren").with_payload(json!({"children": [0]}))); // 1: params
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 2: callee
    ast.push(AstNode::new("var").with_payload(json!("n"))); // 3: arg
    ast.push(AstNode::new("call").with_payload(json!({"children": [2, 3]}))); // 4: f(n)
    ast.push(AstNode::new("function").with_payload(json!({"children": [1, 4]}))); // 5: fn
    ast.push(AstNode::new("var").with_payload(json!("f"))); // 6: target
    ast.push(AstNode::new("assign").with_payload(json!({"children": [6, 5]}))); // 7: assign

    // --- type expression: Total(type.number -> type.number) ---
    let param_fa = push_type_number_expr(&mut ast, 8); // 10
    let body_fa = push_type_number_expr(&mut ast, 11); // 13
    ast.push(AstNode::new("function").with_payload(json!({"children": [param_fa, body_fa]}))); // 14
    ast.push(AstNode::new("var").with_payload(json!("Total"))); // 15
    ast.push(AstNode::new("call").with_payload(json!({"children": [15, 14]}))); // 16

    // --- annotation ---
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [7, 16]}))); // 17

    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("disproved")),
        "expected disproved error, got {:?}",
        err
    );
}

// ===========================================================================
// Phase 8.2: Marshalable meta-predicate
//
// Marshalable(T) asserts that type T is FFI-marshalable (cstr, i32, i64,
// f64, buf, ptr, void, handle) per ADR-0083. It is a meta-predicate handled
// specially in synth_type_annot — the type argument is embedded in the
// annotation's `arg` field, and `is_marshalable()` is called on it.
//
// ffi_bind type checking (Step 3) is deferred until the FFI declaration
// syntax is formalized — there is no `ffi_bind` AST node kind yet. The
// Marshalable predicate provides the building block: once ffi_bind
// declarations exist, each arg/return type can be checked with
// `T : Marshalable`.
// ===========================================================================

/// Build a `type.<name>` field-access type-expression subtree, returning
/// the index of the root `field-access` node. Pushes 3 nodes: var("type"),
/// var(name), field-access{children:[type_idx, name_idx]}.
fn push_type_expr(ast: &mut Ast, offset: usize, type_name: &str) -> usize {
    let type_idx = offset;
    let name_idx = type_idx + 1;
    let fa_idx = type_idx + 2;
    ast.push(AstNode::new("var").with_payload(json!("type"))); // type_idx
    ast.push(AstNode::new("var").with_payload(json!(type_name))); // name_idx
    ast.push(AstNode::new("field-access").with_payload(json!({"children": [type_idx, name_idx]}))); // fa_idx
    fa_idx
}

/// Build a `Marshalable(type.<name>)` call-expression subtree, returning
/// the index of the root `call` node. Pushes the type expr (3 nodes) plus
/// var("Marshalable") and call{children:[marshalable_idx, type_expr_idx]}.
fn push_marshalable_annotation(ast: &mut Ast, offset: usize, type_name: &str) -> usize {
    let type_expr_idx = push_type_expr(ast, offset, type_name); // offset+2
    let marshalable_idx = offset + 3;
    let call_idx = offset + 4;
    ast.push(AstNode::new("var").with_payload(json!("Marshalable"))); // marshalable_idx
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [marshalable_idx, type_expr_idx]})),
    ); // call_idx
    call_idx
}

#[test]
fn marshalable_predicate_proven_for_ffi_type() {
    // `42 : Marshalable(type.i32)` → Proven (type.i32 is FFI-marshalable).
    let engine = engine();
    let lang = meta_predicate_lang();

    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 0: expression
    let annot_idx = push_marshalable_annotation(&mut ast, 1, "i32"); // 5
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [0, annot_idx]}))); // 6

    let req = TypecheckRequest::new(ast);
    let result = engine.typecheck(&lang, &req).unwrap();
    // Proven → annotation accepted, no diagnostics.
    assert!(
        result.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        result.diagnostics
    );
    // The accepted type is the annotation type (type.Marshalable).
    let tag = result.typ.get("__tag__").and_then(|t| t.as_str());
    assert_eq!(tag, Some("type.Marshalable"));
}

#[test]
fn marshalable_predicate_disproved_for_non_ffi_type() {
    // `42 : Marshalable(type.string)` → Disproved (type.string is not
    // FFI-marshalable).
    let engine = engine();
    let lang = meta_predicate_lang();

    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 0: expression
    let annot_idx = push_marshalable_annotation(&mut ast, 1, "string"); // 5
    ast.push(AstNode::new("type-annot").with_payload(json!({"children": [0, annot_idx]}))); // 6

    let req = TypecheckRequest::new(ast);
    let err = engine.typecheck(&lang, &req).unwrap_err();
    assert!(
        matches!(err, LangError::Engine { ref message, .. } if message.contains("disproved")),
        "expected disproved error, got {:?}",
        err
    );
    assert!(
        err.to_string().contains("not FFI-marshalable"),
        "error should mention FFI-marshalable, got: {}",
        err
    );
}

#[test]
fn marshalable_predicate_for_each_ffi_type() {
    // Test each type in FFI_MARSHALLING_TYPES → all Proven.
    // The FFI marshalling types are: cstr, i32, i64, f64, buf, ptr, void,
    // handle (ADR-0083).
    let ffi_types = ["cstr", "i32", "i64", "f64", "buf", "ptr", "void", "handle"];

    for type_name in &ffi_types {
        let engine = engine();
        let lang = meta_predicate_lang();

        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(42))); // 0: expression
        let annot_idx = push_marshalable_annotation(&mut ast, 1, type_name); // 5
        ast.push(AstNode::new("type-annot").with_payload(json!({"children": [0, annot_idx]}))); // 6

        let req = TypecheckRequest::new(ast);
        let result = engine.typecheck(&lang, &req).unwrap_or_else(|e| {
            panic!(
                "Marshalable(type.{}) should be Proven but got error: {}",
                type_name, e
            )
        });
        assert!(
            result.diagnostics.is_empty(),
            "Marshalable(type.{}): expected no diagnostics, got: {:?}",
            type_name,
            result.diagnostics
        );
        let tag = result.typ.get("__tag__").and_then(|t| t.as_str());
        assert_eq!(
            tag,
            Some("type.Marshalable"),
            "Marshalable(type.{}): expected type.Marshalable, got {:?}",
            type_name,
            result.typ
        );
    }
}
