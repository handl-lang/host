//! Arrow-lowered `match` — the pattern shapes `lower_match` compiles to
//! `pair_tag`/`Choice` dispatch must produce the same result as the
//! tree-walk engine's `match_shell`/`try_pattern` walk.
//!
//! For every source the test lowers the desugared AST via
//! `handl_kit_arrow_lower::lower_ast`, interprets the `Term`, and
//! compares against `eval_with_globals` (which bypasses the lowerer) —
//! a decline or a mismatch fails the test. `__tag__` scrutinees cannot
//! be written as object literals (`__tag__` is a reserved key), so the
//! tag-pattern cases hand-build the arena like the tree-walk tests do.

use handl_kit_arrow_lower::lower_ast;
use handl_language::{Ast, AstNode, AstRef, EvalSchemeDef, Source, Term, Value};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use plugin_engine::term_interpreter::interpret;
use serde_json::json;
use std::collections::HashMap;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

/// `src` must lower *and* interpret to the same value the bound eval
/// engine produces — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let got = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    let want = rt
        .eval_with_globals(&ast, &HashMap::new())
        .unwrap_or_else(|e| panic!("engine eval failed on {src:?}: {e}"));
    assert_eq!(got, want, "{src:?}");
}

/// `src` must *decline* to lower — the AST eval fallback covers the
/// semantics the arrow lowering cannot express. Currently unused —
/// the runtime-computed-key decline lifted when `has_field`/
/// `field_access`/`obj_except` grew their two-arg operand forms — kept
/// for the next pattern shape the lowerer cannot express.
#[allow(dead_code)]
fn assert_declines(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    assert!(
        lower_ast(&ast, rt.lang(), root).is_err(),
        "{src:?} should decline"
    );
}

/// Like [`assert_lowered`] but for a hand-built arena: the lowered term
/// must interpret to `want`.
fn assert_lowered_ast(ast: &Ast, want: Value) {
    let rt = runtime();
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined hand-built ast: {e}"));
    let got =
        interpret(&term, Value::Null).unwrap_or_else(|e| panic!("term interpreter failed: {e}"));
    assert_eq!(got, want);
}

// ---------------------------------------------------------------------------
// Compiled backends — bytecode + cranelift run the lowered `Term` itself.
// ---------------------------------------------------------------------------

/// Run `term` through a *compiled* engine end-to-end: it is registered
/// as the eval scheme of a synthetic `test-lowered` root node, so
/// `engine.eval` compiles the term (`compile`/`compile_arr` →
/// `NativeOp`/`apply_native` → `native_ops::apply_native_op`) and
/// executes it with a `Null` incoming — exactly the term-interpreter's
/// `interpret(&term, Value::Null)` inputs. There is no `match_shell`/
/// `try_pattern` AST fallback on this path: an unimplemented op errors
/// the eval instead.
fn eval_term_on(engine: &dyn EvalEngine, lang: &handl_language::Language, term: &Term) -> Value {
    let mut lang = lang.clone();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.test.lowered",
            "test-lowered",
            term.clone(),
        ))
        .expect("merge test scheme");
    let mut ast = Ast::new();
    ast.push(AstNode::new("test-lowered"));
    engine
        .eval(&lang, &EvalRequest::new(ast))
        .unwrap_or_else(|e| panic!("compiled engine failed on lowered term: {e}"))
}

/// The lowered `Term` for `src` must produce the term-interpreter's
/// result on the bytecode and cranelift backends — proving the
/// compiled-backend `native_ops` op set (and its `op:<json-lit>` name
/// encoding) covers every op the lowering emits.
fn assert_lowered_on_compiled_engines(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let want = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    for (name, engine) in [
        ("bytecode", Box::new(BytecodeEngine::new()) as Box<dyn EvalEngine>),
        ("cranelift", Box::new(CraneliftEngine::new()) as _),
    ] {
        let got = eval_term_on(engine.as_ref(), rt.lang(), &term);
        assert_eq!(got, want, "{name} on {src:?}");
    }
}

/// Hand-built-arena counterpart of [`assert_lowered_on_compiled_engines`]
/// (tag patterns need `__tag__` scrutinees object literals cannot write).
fn assert_lowered_ast_on_compiled_engines(ast: &Ast, want: Value) {
    let rt = runtime();
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined hand-built ast: {e}"));
    for (name, engine) in [
        ("bytecode", Box::new(BytecodeEngine::new()) as Box<dyn EvalEngine>),
        ("cranelift", Box::new(CraneliftEngine::new()) as _),
    ] {
        let got = eval_term_on(engine.as_ref(), rt.lang(), &term);
        assert_eq!(got, want, "{name} on hand-built ast");
    }
}

#[test]
fn object_patterns_on_compiled_engines() {
    // Static keys, `...rest`, and fallthrough — `has_field`/`field_access`/
    // `obj_except` static-lit forms plus `elem` (all ride the compiled
    // backends' `op:<json-lit>` name encoding).
    assert_lowered_on_compiled_engines(
        r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r.b, _: 0 }"#,
    );
    assert_lowered_on_compiled_engines(
        r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r, _: 0 }"#,
    );
    assert_lowered_on_compiled_engines(r#"match { "a": 5 } { { "c": x }: x, _: 0 }"#);
    assert_lowered_on_compiled_engines(r#"match 5 { { "a": x }: x, _: 0 }"#);
    // `{}` — the `is_obj` discriminant.
    assert_lowered_on_compiled_engines(r#"match { "a": 1 } { {}: "obj", _: "no" }"#);
    assert_lowered_on_compiled_engines(r#"match 5 { {}: "obj", _: "no" }"#);
    // Nested object/array sub-patterns — `arr_len`/`arr_drop` too.
    assert_lowered_on_compiled_engines(
        r#"match { "a": [1, 2] } { { "a": [x, ...t] }: x, _: 0 }"#,
    );
    // Literal sub-patterns under entries + `has_field` guard fallthrough.
    assert_lowered_on_compiled_engines(
        r#"match { "a": 5, "b": 2 } { { "a": 5 }: "hit", _: "miss" }"#,
    );
    assert_lowered_on_compiled_engines(
        r#"match { "a": 5, "b": 2 } { { "a": 6 }: "hit", _: "miss" }"#,
    );
}

#[test]
fn runtime_object_keys_on_compiled_engines() {
    // Runtime-valued computed keys — `obj_key` coercion, the `array`/
    // `product` slot-2 key list, and the two-arg dynamic forms of
    // `has_field`/`field_access`/`obj_except`.
    assert_lowered_on_compiled_engines(
        r#"do { o :: {"k": "ab"}, match { "ab": 7 } { {[o.k]: x}: x, _: 0 } }"#,
    );
    assert_lowered_on_compiled_engines(
        r#"do { o :: {"k": "ab"}, match { "zz": 7 } { {[o.k]: x}: x, _: 0 } }"#,
    );
    // `...rest` under a runtime key — `obj_except(target, keys_list)`.
    assert_lowered_on_compiled_engines(
        r#"do { o :: {"k": "ab"}, match { "ab": 7, "c": 3 } { {[o.k]: x, ...r}: r.c, _: 0 } }"#,
    );
    // Non-object scrutinee: `is_obj` gates the key-forcing stage.
    assert_lowered_on_compiled_engines(
        r#"do { o :: {"k": "ab"}, match 5 { {[o.k]: x}: x, _: 0 } }"#,
    );
    // Env-bound constant key — `static_term_value` collapses it to the
    // static `lit` forms.
    assert_lowered_on_compiled_engines(
        r#"do { k :: "a", match { "b": 1, "a": 2 } { {[k]: x, ...r}: r.b, _: 0 } }"#,
    );
    // Non-string runtime key coerces to `""` (try_object_pattern parity).
    assert_lowered_on_compiled_engines(
        r#"do { n :: 42, match { "a": 1 } { {[n]: x}: x, _: 0 } }"#,
    );
}

#[test]
fn tag_patterns_on_compiled_engines() {
    // `has_field "__tag__"` guard + `tag_fields` positional binders.
    let ast = ok_err_ast(json!({ "__tag__": "ok", "value": 42 }));
    assert_lowered_ast_on_compiled_engines(&ast, json!(42));
    let ast = ok_err_ast(json!({ "__tag__": "err", "value": 99 }));
    assert_lowered_ast_on_compiled_engines(&ast, json!(0));
    // Multi-binder positional: `Pair(a, b)` binds sorted payload fields.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!({ "__tag__": "Pair", "x": 10, "y": 20 })));
    ast.push(var("Pair")); // 1
    ast.push(var("a")); // 2
    ast.push(var("b")); // 3
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2, 3] }))); // 4
    ast.push(var("a")); // 5
    ast.push(var("b")); // 6
    ast.push(AstNode::new("array").with_payload(json!({ "children": [5, 6] }))); // 7
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [4, 7] }))); // 8
    ast.push(var("_")); // 9
    ast.push(AstNode::new("value").with_payload(json!(0))); // 10
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [9, 10] }))); // 11
    ast.push(AstNode::new("block").with_payload(json!({ "children": [8, 11] }))); // 12
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 12] }))); // 13
    assert_lowered_ast_on_compiled_engines(&ast, json!([10, 20]));
}

// ---------------------------------------------------------------------------
// Structural patterns — the pre-existing set, kept honest.
// ---------------------------------------------------------------------------

#[test]
fn literal_and_binder_patterns() {
    assert_lowered(r#"match 2 { 1: "one", 2: "two", _: "other" }"#);
    assert_lowered(r#"match "x" { "x": 1, _: 0 }"#);
    assert_lowered(r#"match 1.0 { 1: "hit", _: "miss" }"#); // cross-numeric eq
    assert_lowered(r#"match null { null: "n", _: "z" }"#);
    assert_lowered(r#"match 7 { x: x }"#); // bare binder
}

#[test]
fn global_value_patterns() {
    // `true`/`false` are `Language::globals` names — they compare by
    // equality in pattern position *and* resolve to their value in
    // scrutinee position.
    assert_lowered(r#"match true { true: "yes", false: "no", _: "z" }"#);
    assert_lowered(r#"match false { true: "yes", false: "no", _: "z" }"#);
    assert_lowered(r#"match 1 { true: "yes", false: "no", _: "z" }"#);
}

#[test]
fn array_patterns() {
    assert_lowered(r#"match [1, 2] { []: 0, [h, ...t]: h, _: 9 }"#);
    assert_lowered(r#"match [] { []: 0, [h, ...t]: h, _: 9 }"#);
    assert_lowered(r#"match "abc" { []: 0, _: 9 }"#); // non-array falls through
    assert_lowered(r#"match [1, 2, 3] { [a, b, c]: a + b + c, _: 0 }"#);
    assert_lowered(r#"match [1, 2, 3] { [...t]: t, _: 0 }"#); // rest-only
}

#[test]
fn array_patterns_with_sub_patterns() {
    // Literal elements, nested arrays, wildcards — the decline set that
    // drove `["-", ...t]` in `desugar.hnl` to the AST fallback.
    assert_lowered(r#"match ["-", "a"] { ["-", ...t]: t, _: [] }"#);
    assert_lowered(r#"match [] { ["-", ...t]: 1, _: 0 }"#);
    assert_lowered(r#"match [1, 2] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 3] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [[1, 2], [3]] { [[a, b], ...t]: a + b, _: 0 }"#);
    assert_lowered(r#"match [[1]] { [[a, b], ...t]: a + b, _: 0 }"#);
    assert_lowered(r#"match [[1, 2]] { [[a, 2]]: a, _: 0 }"#);
    assert_lowered(r#"match [["x"]] { [["x"]]: 1, _: 0 }"#);
    // A too-short scrutinee falls through rather than erroring.
    assert_lowered(r#"match [1] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 2, 3] { [1, _, 3]: "hit", _: "miss" }"#);
}

#[test]
fn object_patterns() {
    assert_lowered(r#"match { "a": 1 } { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match 5 { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match [] { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": 5 }: "hit", _: "miss" }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": 6 }: "hit", _: "miss" }"#);
    // Missing key / non-object falls through — `try_object_pattern`'s
    // `Ok(false)`, not an error.
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "c": x }: x, _: 0 }"#);
    assert_lowered(r#"match 5 { { "a": x }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r.b, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r, _: 0 }"#);
    // Nested sub-patterns under entries.
    assert_lowered(r#"match { "k": { "v": 7 } } { { "k": { "v": x } }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": [1, 2] } { { "a": [x, ...t] }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": [1] } { { "a": [x, ...t] }: x, _: 0 }"#);
}

#[test]
fn tag_record_patterns() {
    // `{_tag: "Some", value: x}` — the `nullish`/`opt-chain` desugar
    // shape, plus quoted keys routed through the general entries path.
    assert_lowered(
        r#"match { "_tag": "Some", "value": 5 } { { "_tag": "Some", "value": x }: x, _: 0 }"#,
    );
    assert_lowered(r#"match { "_tag": "None" } { { "_tag": "Some", "value": x }: x, _: 0 }"#);
}

#[test]
fn desugared_match_patterns() {
    // `??`/`?.` desugar to `match` on `{_tag: ...}` records with a
    // `__fresh_` payload binder — the wildcard filter must bind those
    // names, not drop them.
    assert_lowered(r#"null ?? 7"#);
    assert_lowered(r#"5 ?? 7"#);
    assert_lowered(r#"{ "_tag": "Some", "value": 7 } ?? 9"#);
}

#[test]
fn arm_bodies_see_rebased_env() {
    // Regression: a binding lowered to consume the node's input `in`
    // must be re-anchored through `snd` of the `[sv, in]` dispatch pair —
    // previously `y` evaluated to the pair itself.
    assert_lowered(r#"((xs) ~> do { y :: xs, match xs { []: y, _: 99 } })([])"#);
    assert_lowered(r#"((xs) ~> do { y :: xs, match xs { []: y, _: 99 } })([1])"#);
    assert_lowered(r#"((xs) ~> match xs { []: 1, _: 2 })([])"#);
    assert_lowered(r#"((xs) ~> match xs { []: 1, _: 2 })([3])"#);
    assert_lowered(r#"do { f :: (xs) ~> match xs { []: 1, _: 2 }, f([]) }"#);
}

#[test]
fn paren_literal_eval_patterns() {
    // `(expr)` in pattern position is `try_pattern`'s literal-eval tail:
    // evaluate the expression in the ambient env and `eq`-compare. The
    // `[sv, in]` pair feeds the lowered expression `in`, so env bindings
    // *and* enclosing params resolve — not just literals.
    assert_lowered(r#"match 5 { (5): "hit", _: "miss" }"#);
    assert_lowered(r#"match 4 { (5): "hit", _: "miss" }"#);
    assert_lowered(r#"match 3 { (1 + 2): "hit", _: "miss" }"#);
    assert_lowered(r#"match 2 { (1 + 2): "hit", _: "miss" }"#);
    // Env-bound constants and param values both resolve.
    assert_lowered(r#"do { n :: 5, match 5 { (n): "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 5, match 4 { (n): "hit", _: "miss" } }"#);
    assert_lowered(r#"((n) ~> match 5 { (n): "hit", _: "miss" })(5)"#);
    assert_lowered(r#"((n) ~> match 5 { (n): "hit", _: "miss" })(4)"#);
    // Compound expressions — field access, literals.
    assert_lowered(r#"do { o :: {"a": 7}, match 7 { (o.a): "hit", _: "miss" } }"#);
    assert_lowered(r#"match [1, 2] { ([1, 2]): "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 3] { ([1, 2]): "hit", _: "miss" }"#);
    // Literal-eval *nested* inside structural patterns — `in` reaches
    // any depth through the re-paired `[v', in]` sub-scrutinee.
    assert_lowered(r#"do { n :: 2, match [1, 2] { [1, (n)]: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 2, match [1, 3] { [1, (n)]: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 2, match { "a": 2 } { { "a": (n) }: "hit", _: "miss" } }"#);
    assert_lowered(r#"((n) ~> match { "a": 9 } { { "a": (n) }: "hit", _: "miss" })(9)"#);
    assert_lowered(r#"((n) ~> match { "a": 9 } { { "a": (n) }: "hit", _: "miss" })(8)"#);
}

#[test]
fn computed_object_pattern_keys() {
    // `[expr]`/`(expr)` entry keys desugar to `paren` key children — a
    // computed key the interpreter evaluates in the ambient env. When
    // that expression is a compile-time constant (an env-bound constant
    // like `k :: "a"`), the key string resolves statically.
    assert_lowered(r#"do { k :: "a", match { "a": 1 } { {[k]: x}: x, _: 0 } }"#);
    assert_lowered(r#"do { k :: "a", match { "b": 1, "a": 2 } { {[k]: x}: x, _: 0 } }"#);
    // `...rest` excludes the statically-resolved matched key.
    assert_lowered(r#"do { k :: "a", match { "b": 1, "a": 2 } { {[k]: x, ...r}: r.b, _: 0 } }"#);
    assert_lowered(r#"do { k :: "a", match { "b": 1 } { {[k]: x}: x, _: 0 } }"#);
    // `paren`-wrapped literal keys keep working through the same path.
    assert_lowered(r#"match { "a": 1 } { {("a"): x}: x, _: 0 }"#);
}

#[test]
fn runtime_object_pattern_keys() {
    // A computed key whose value is genuinely runtime (a param) lowers
    // through the two-arg operand forms of `has_field`/`field_access`:
    // `try_object_pattern` evaluates the key expression in the ambient
    // env per match attempt, so the lowered pair carries the keys in a
    // slot-2 list forced once ahead of the discriminant chain.
    assert_lowered(r#"((k) ~> match { "a": 1 } { {[k]: x}: x, _: 0 })("a")"#);
    // Key misses the object → arm falls through to the wildcard.
    assert_lowered(r#"((k) ~> match { "a": 1 } { {[k]: x}: x, _: 0 })("zz")"#);
    // Scrutinee that is not an object: `is_obj` gates the key-forcing
    // stage so no key term evaluates (mirrors try_object_pattern's
    // early `Ok(false)` on non-Obj before key collection).
    assert_lowered(r#"((k) ~> match 5 { {[k]: x}: x, _: 0 })("a")"#);
    // Param key + sub-pattern under the dynamic field.
    assert_lowered(
        r#"((k) ~> match { "a": 9 } { {[k]: 9}: "hit", _: "miss" })("a")"#,
    );
    assert_lowered(r#"((k) ~> match { "a": 9 } { {[k]: 9}: "hit", _: "miss" })("b")"#);
    // Env-bound *runtime* key — a non-const expression (field access on
    // a bound record), which `static_term_value` cannot collapse.
    assert_lowered(
        r#"do { o :: {"k": "ab"}, match { "ab": 7 } { {[o.k]: x}: x, _: 0 } }"#,
    );
    // Nested object pattern with a runtime key one level down.
    assert_lowered(
        r#"((k) ~> match { "a": { "b": 7 } } { { "a": {[k]: x} }: x, _: 0 })("b")"#,
    );
    assert_lowered(
        r#"((k) ~> match { "a": { "b": 7 } } { { "a": {[k]: x} }: x, _: 0 })("z")"#,
    );
    // `...rest` under a runtime key — `obj_except(target, keys)` reads
    // the slot-2 evaluated key list.
    assert_lowered(
        r#"((k) ~> match { "b": 1, "a": 2 } { {[k]: x, ...r}: r.b, _: 0 })("a")"#,
    );
    assert_lowered(
        r#"((k) ~> match { "b": 1, "a": 2 } { {[k]: x, ...r}: r, _: 0 })("a")"#,
    );
    // Mixed static + runtime keys in one pattern.
    assert_lowered(
        r#"((k) ~> match { "a": 1, "b": 2 } { { "a": 1, [k]: x }: x, _: 0 })("b")"#,
    );
    assert_lowered(
        r#"((k) ~> match { "a": 1, "b": 2 } { { "a": 1, [k]: x }: x, _: 0 })("c")"#,
    );
    // Key hitting an absent field: arm falls through to wildcard.
    assert_lowered(r#"((k) ~> match { "a": 1 } { {[k]: x}: x, _: 5 })("b")"#);
    // Non-string runtime key coerces to `""` (try_object_pattern parity).
    assert_lowered(r#"((k) ~> match { "a": 1 } { {[k]: x}: x, _: 0 })(42)"#);
    assert_lowered(r#"((k) ~> match { "": 3, "a": 1 } { {[k]: x}: x, _: 0 })(42)"#);
    // Runtime key under a nested literal-eval sub-pattern.
    assert_lowered(
        r#"((k) ~> match { "a": 9 } { {[k]: (9)}: "hit", _: "miss" })("a")"#,
    );
    // Runtime key over a non-object scrutinee falls through cleanly.
    assert_lowered(r#"((k) ~> match [1, 2] { {[k]: x}: x, _: 0 })("a")"#);
    // A dynamic-key binder used *twice* in the body — the key list is
    // forced once ahead of the discriminant and rides the dispatched
    // `[v, in, keys]` triple into the body; the binder reads `keys[i]`
    // rather than re-evaluating the key term per use.
    assert_lowered(
        r#"((k) ~> match { "a": 9 } { {[k]: x}: x + x + x, _: 0 })("a")"#,
    );
    assert_lowered(
        r#"((k) ~> match { "a": 9 } { {[k]: x}: [x, x, x], _: 0 })("a")"#,
    );
    // Two dynamic keys, both binders used, plus a `...rest` read —
    // via do-block slots (`(k, j) ~>` multi-param dynamic keys are a
    // pre-existing `elem` OOB on the pair projection, unrelated to the
    // prologue split).
    assert_lowered(
        r#"do { k :: "a", j :: "b", match { "a": 1, "b": 2, "c": 3 } { {[k]: x, [j]: y, ...r}: x + y + r.c, _: 0 } }"#,
    );
}

/// Count `Op{name}` occurrences in a term — the structural witness that
/// a runtime-valued key expression is emitted *once* (evaluated once per
/// arm attempt into the slot-2 key list), not once per discriminant walk
/// plus once per binder-path use.
fn count_op(t: &Term, name: &str) -> usize {
    let mut n = 0;
    // Quoted arrows (`Arr` carrying a serialized `Term`) hide their body
    // in the payload — descend so ops inside a bound function count too.
    if let Term::Arr { value, .. } = t {
        if let Ok(inner) = serde_json::from_value::<Term>(value.clone()) {
            n += count_op(&inner, name);
        }
    }
    if let Term::Op {
        name: op,
        args,
        dim,
        ..
    } = t
    {
        if op == name {
            n += 1;
        }
        n += count_op(dim, name);
        for a in args {
            n += count_op(a, name);
        }
    }
    match t {
        Term::Seq { axis, fst, snd } => {
            n += count_op(axis, name) + count_op(fst, name) + count_op(snd, name)
        }
        Term::First { inner } => n += count_op(inner, name),
        Term::Choice { l, r } => n += count_op(l, name) + count_op(r, name),
        Term::Loop { body } => n += count_op(body, name),
        Term::App { callee, args } => {
            n += count_op(callee, name);
            for a in args {
                n += count_op(a, name);
            }
        }
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => {
            n += count_op(fuel, name)
                + count_op(predicate, name)
                + count_op(step, name)
                + count_op(initial, name)
        }
        Term::Handle { body, impl_, .. } => n += count_op(body, name) + count_op(impl_, name),
        _ => {}
    }
    n
}

/// Lower `src` and count `obj_key` ops in the emitted term.
fn obj_key_count(src: &str) -> usize {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    count_op(&term, "obj_key")
}

#[test]
fn runtime_object_key_evaluated_once() {
    // One runtime key whose binder is used three times: the `obj_key`
    // coercion (and thus the key term under it) appears exactly once in
    // the lowered term — the arm prologue's `extend` — instead of once
    // in the discriminant plus once per binder path.
    assert_eq!(
        obj_key_count(r#"((k) ~> match { "a": 9 } { {[k]: x}: x + x + x, _: 0 })("a")"#),
        1,
    );
    // Same under a `...rest` binder — `obj_except` reads the shared
    // slot-2 list rather than re-forcing the keys.
    assert_eq!(
        obj_key_count(r#"((k) ~> match { "a": 1, "b": 2 } { {[k]: x, ...r}: [x, r], _: 0 })("a")"#),
        1,
    );
    // Two dynamic keys → two `obj_key` ops total (one per key), shared
    // between the discriminant walk and both binder paths. Do-block
    // bound keys stay runtime-valued (`o.k` does not static-fold).
    assert_eq!(
        obj_key_count(
            r#"do { o :: {"k": "a", "j": "b"}, match { "a": 1, "b": 2 } { {[o.k]: x, [o.j]: y}: x + y, _: 0 } }"#,
        ),
        2,
    );
}

#[test]
fn literal_eval_tail_patterns() {
    // `try_pattern`'s tail evaluates any non-structural pattern node as
    // an expression — a bare field-access, an object literal, a nested
    // `paren`.
    assert_lowered(r#"do { o :: {"a": 7}, match 7 { o.a: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { o :: {"a": 7}, match 8 { o.a: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { p :: [9], match 9 { (p.0): "hit", _: "miss" } }"#);
}

#[test]
fn match_fail_on_total_miss() {
    // No arm matches → the interpreter raises "match: no arm matched".
    // The lowered `match_fail` op errors the same way; `Runtime::eval`
    // then falls back to the engine, which reports it identically.
    let rt = runtime();
    let src = r#"match 3 { 1: "one", 2: "two" }"#;
    let toks = rt.lex(&Source::new(src)).unwrap();
    let ast = rt.parse(&toks).unwrap();
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root).expect("must lower");
    let err = interpret(&term, Value::Null).unwrap_err();
    assert!(err.contains("no arm matched"), "{err}");
    assert!(rt.eval_with_globals(&ast, &HashMap::new()).is_err());
}

// ---------------------------------------------------------------------------
// Tag patterns (ADR-0077) — `call` patterns on `__tag__` payloads.
// ---------------------------------------------------------------------------

fn var(name: &str) -> AstNode {
    AstNode::new("var").with_payload(json!(name))
}

/// `match scrut { ok(v): v, err(e): 0 }` — two tag-pattern arms.
fn ok_err_ast(scrut: Value) -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(scrut)); // 0 scrutinee
    ast.push(var("ok")); // 1
    ast.push(var("v")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3 pat ok(v)
    ast.push(var("v")); // 4 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [3, 4] }))); // 5
    ast.push(var("err")); // 6
    ast.push(var("e")); // 7
    ast.push(AstNode::new("call").with_payload(json!({ "children": [6, 7] }))); // 8 pat err(e)
    ast.push(AstNode::new("value").with_payload(json!(0))); // 9 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [8, 9] }))); // 10
    ast.push(AstNode::new("block").with_payload(json!({ "children": [5, 10] }))); // 11
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 11] }))); // 12
    ast
}

#[test]
fn tag_pattern_binds_payload() {
    let ast = ok_err_ast(json!({ "__tag__": "ok", "value": 42 }));
    assert_lowered_ast(&ast, json!(42));
}

#[test]
fn tag_pattern_falls_through_on_tag_miss() {
    let ast = ok_err_ast(json!({ "__tag__": "err", "value": 99 }));
    assert_lowered_ast(&ast, json!(0));
}

#[test]
fn tag_pattern_no_match_errors() {
    // A non-object scrutinee misses every tag arm → `match_fail` — the
    // same "no arm matched" error `match_shell` raises (the tree-walk
    // `match_tag_pattern_non_tagged_value_no_match` case).
    let rt = runtime();
    let ast = ok_err_ast(json!(42));
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root).expect("must lower");
    let err = interpret(&term, Value::Null).unwrap_err();
    assert!(err.contains("no arm matched"), "{err}");
}

#[test]
fn tag_pattern_multi_binder_positional() {
    // `Pair(a, b)` binds the sorted non-`__tag__` payload values
    // positionally — `{x, y}` yields `a=x, b=y` (alphabetical order).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!({ "__tag__": "Pair", "x": 10, "y": 20 }))); // 0
    ast.push(var("Pair")); // 1
    ast.push(var("a")); // 2
    ast.push(var("b")); // 3
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2, 3] }))); // 4 pat
    ast.push(var("a")); // 5
    ast.push(var("b")); // 6
    ast.push(AstNode::new("array").with_payload(json!({ "children": [5, 6] }))); // 7 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [4, 7] }))); // 8
    ast.push(var("_")); // 9 wildcard pat
    ast.push(AstNode::new("value").with_payload(json!(0))); // 10 fallback body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [9, 10] }))); // 11
    ast.push(AstNode::new("block").with_payload(json!({ "children": [8, 11] }))); // 12
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 12] }))); // 13
    assert_lowered_ast(&ast, json!([10, 20]));
}

#[test]
fn tag_pattern_wildcard_and_zero_binder() {
    // `ok(_)` matches the tag and binds nothing; `ok()` the same.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!({ "__tag__": "ok", "value": 1 }))); // 0
    ast.push(var("ok")); // 1
    ast.push(var("_")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3
    ast.push(AstNode::new("value").with_payload(json!("hit"))); // 4
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [3, 4] }))); // 5
    ast.push(AstNode::new("block").with_payload(json!({ "children": [5] }))); // 6
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 6] }))); // 7
    assert_lowered_ast(&ast, json!("hit"));
}

#[test]
fn nested_tag_pattern_inside_array() {
    // `[ok(v), ...t]` — a tag pattern nested inside an array pattern.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!([{ "__tag__": "ok", "value": 8 }, 0]))); // 0 scrutinee
    ast.push(var("ok")); // 1
    ast.push(var("v")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3 ok(v)
    ast.push(var("t")); // 4
    ast.push(AstNode::new("rest").with_payload(json!({ "children": [4] }))); // 5 ...t
    ast.push(AstNode::new("array").with_payload(json!({ "children": [3, 5] }))); // 6 [ok(v), ...t]
    ast.push(var("v")); // 7 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [6, 7] }))); // 8
    ast.push(AstNode::new("block").with_payload(json!({ "children": [8] }))); // 9
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 9] }))); // 10
    assert_lowered_ast(&ast, json!(8));
}
