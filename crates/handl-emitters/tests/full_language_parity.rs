//! `full_language_parity` integration test (ADR-0044 Phase 4).
//!
//! Merges all subset emitters into one `Language` and runs a shared test
//! corpus through the data-driven engine (Pratt for parse where applicable,
//! tree-walk for eval). Asserts the final values match expectations.
//!
//! ## Scope
//!
//! This test covers the **representative subset** of migrated constructs:
//! identifier, value, arith (+,-,*,/), let, call (2-arg), branch (if), and
//! function literals. The remaining ~43 constructs follow the same pattern
//! and are deferred.
//!
//! ## Payload convention
//!
//! The data-driven Pratt engine produces ASTs with `{"children": [...]}` for
//! compound nodes and token values for atoms. Some constructs (let, call,
//! branch, function) require payload metadata (e.g. `name`, `params`, `body`)
//! that the Pratt engine cannot produce. For those constructs, ASTs are
//! constructed manually with the correct payload shape. Simple expressions
//! (arith, atoms) are parsed by the Pratt engine. This is expected — the
//! Pratt engine's grammar row format is limited and will be enhanced in
//! future work. The **eval schemes** are the fully migrated part.

use handl_emitter_arith;
use handl_emitter_branch;
use handl_emitter_call;
use handl_emitter_function;
use handl_emitter_identifier;
use handl_emitter_let;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language, Token, Tokens};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use serde_json::json;

/// Merge all subset emitters into one complete `Language`.
fn full_language() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(handl_emitter_identifier::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_let::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_call::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_branch::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_function::emit().unwrap()).unwrap();
    lang
}

/// Helper: make a token.
fn tok(kind: &str, text: &str) -> Token {
    Token {
        kind: kind.into(),
        text: text.into(),
    }
}

/// Helper: make a token stream.
fn toks(parts: &[(&str, &str)]) -> Tokens {
    Tokens(parts.iter().map(|(k, t)| tok(k, t)).collect())
}

/// Helper: make a function node with params and body index.
fn func_node(ast: &mut Ast, params: &[&str], body_idx: usize) -> usize {
    let params_json: Vec<serde_json::Value> = params.iter().map(|p| json!(p)).collect();
    let idx = ast.len();
    ast.push(
        AstNode::new("function").with_payload(json!({
            "params": params_json,
            "body": body_idx,
        })),
    );
    idx
}

// ---------------------------------------------------------------------------
// Test corpus
// ---------------------------------------------------------------------------

/// `1 + 2` → 3 (Pratt parse + tree-walk eval)
#[test]
fn corpus_arith_add() {
    let lang = full_language();
    let parse_engine = PrattEngine::new();
    let eval_engine = TreeWalkEngine::new();

    let ast = parse_engine
        .parse(
            &lang,
            &ParserRequest::new(toks(&[
                ("token.number", "1"),
                ("token.plus", "+"),
                ("token.number", "2"),
            ])),
        )
        .unwrap();
    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(3));
}

/// `1 + 2 * 3` → 7 (precedence)
#[test]
fn corpus_arith_precedence() {
    let lang = full_language();
    let parse_engine = PrattEngine::new();
    let eval_engine = TreeWalkEngine::new();

    let ast = parse_engine
        .parse(
            &lang,
            &ParserRequest::new(toks(&[
                ("token.number", "1"),
                ("token.plus", "+"),
                ("token.number", "2"),
                ("token.star", "*"),
                ("token.number", "3"),
            ])),
        )
        .unwrap();
    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(7));
}

/// `let x = 1 + 2; x` → 3 (manual AST — let requires payload metadata)
#[test]
fn corpus_let_with_arith() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.add([0, 1]) — 1 + 2
    // Node 3: var("x") — body
    // Node 4: let({name:"x", children:[2, 3]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1))); // 0
    ast.push(AstNode::new("value").with_payload(json!(2))); // 1
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 2
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 3
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [2, 3]})),
    ); // 4

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(3));
}

/// `if (true) 1 else 2` → 1 (manual AST — if requires 3 children)
#[test]
fn corpus_if_true() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true))); // 0
    ast.push(AstNode::new("value").with_payload(json!(1))); // 1
    ast.push(AstNode::new("value").with_payload(json!(2))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(1));
}

/// `if (false) 1 else 2` → 2
#[test]
fn corpus_if_false() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(false))); // 0
    ast.push(AstNode::new("value").with_payload(json!(1))); // 1
    ast.push(AstNode::new("value").with_payload(json!(2))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(2));
}

/// `f(1, 2)` where `f = (x, y) -> x + y` → 3 (manual AST)
#[test]
fn corpus_call_2_args() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    // Node 0: var("x") — body of function
    // Node 1: var("y")
    // Node 2: binary.add([0, 1]) — x + y
    // Node 3: function({params:["x","y"], body:2})
    // Node 4: value(1)
    // Node 5: value(2)
    // Node 6: call({children:[3, 4, 5]})
    // Node 7: let({name:"f", children:[3, 6]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 1
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 2
    let func_idx = func_node(&mut ast, &["x", "y"], 2); // 3
    ast.push(AstNode::new("value").with_payload(json!(1))); // 4
    ast.push(AstNode::new("value").with_payload(json!(2))); // 5
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 4, 5]})),
    ); // 6
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "f", "children": [func_idx, 6]})),
    ); // 7

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(3));
}

/// Combined: `let x = 1 + 2; let y = 3; if (true) x + y else 0` → 6
#[test]
fn corpus_combined_let_arith_if() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.add([0, 1]) — 1 + 2 = 3
    // Node 3: var("x") — body of let_x (returns x after binding)
    //    Wait, we need: let x = 3 in (let y = 3 in if(true, x+y, 0))
    //
    // Let me restructure:
    // let x = 1 + 2 in
    //   let y = 3 in
    //     if true then x + y else 0
    //
    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.add([0, 1])     — 1 + 2
    // Node 3: value(3)               — y's value
    // Node 4: var("x")
    // Node 5: var("y")
    // Node 6: binary.add([4, 5])     — x + y
    // Node 7: value(0)               — else branch
    // Node 8: if({children:[?, 6, 7]}) — need a true condition
    // Node 9: value(true)            — condition
    //    Actually, let me reorder:
    //
    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.add([0, 1])     — 1 + 2 = value of x
    // Node 3: value(3)               — value of y
    // Node 4: value(true)            — condition
    // Node 5: var("x")
    // Node 6: var("y")
    // Node 7: binary.add([5, 6])     — x + y
    // Node 8: value(0)               — else
    // Node 9: if({children:[4, 7, 8]})  — if(true, x+y, 0)
    // Node 10: let({name:"y", children:[3, 9]})  — let y = 3 in <if>
    // Node 11: let({name:"x", children:[2, 10]}) — let x = 3 in <let_y>
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1))); // 0
    ast.push(AstNode::new("value").with_payload(json!(2))); // 1
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 2
    ast.push(AstNode::new("value").with_payload(json!(3))); // 3
    ast.push(AstNode::new("value").with_payload(json!(true))); // 4
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 5
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 6
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [5, 6]})),
    ); // 7
    ast.push(AstNode::new("value").with_payload(json!(0))); // 8
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [4, 7, 8]})),
    ); // 9
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "y", "children": [3, 9]})),
    ); // 10
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [2, 10]})),
    ); // 11

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // x = 1+2 = 3, y = 3, if(true) x+y = 6
    assert_eq!(result, json!(6));
}

/// Function with capture: `let z = 10; let f = (x, y) -> x + y + z; f(1, 2)` → 13
#[test]
fn corpus_function_with_capture() {
    let lang = full_language();
    let eval_engine = TreeWalkEngine::new();

    // Node 0: var("x")
    // Node 1: var("y")
    // Node 2: var("z")         — captured
    // Node 3: binary.add([0, 1])  — x + y
    // Node 4: binary.add([3, 2])  — (x + y) + z
    // Node 5: function({params:["x","y"], body:4})
    // Node 6: value(1)
    // Node 7: value(2)
    // Node 8: call({children:[5, 6, 7]})
    // Node 9: value(10)
    // Node 10: let({name:"f", children:[5, 8]})
    // Node 11: let({name:"z", children:[9, 10]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 1
    ast.push(AstNode::new("var").with_payload(json!("z"))); // 2
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 3
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [3, 2]})),
    ); // 4
    let func_idx = func_node(&mut ast, &["x", "y"], 4); // 5
    ast.push(AstNode::new("value").with_payload(json!(1))); // 6
    ast.push(AstNode::new("value").with_payload(json!(2))); // 7
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 6, 7]})),
    ); // 8
    ast.push(AstNode::new("value").with_payload(json!(10))); // 9
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "f", "children": [func_idx, 8]})),
    ); // 10
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "z", "children": [9, 10]})),
    ); // 11

    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(13));
}

/// Verify the merged language has all expected fragments.
#[test]
fn full_language_has_all_fragments() {
    let lang = full_language();
    // Tokens: number, string, bool, null, identifier, plus, minus, star, slash, pow,
    //         let, lparen, rparen, if, arrow
    assert_eq!(lang.tokens.len(), 15);
    // Grammar: value.prefix (number, string, bool, null), identifier.prefix,
    //          add.infix, sub.infix, mul.infix, div.infix, pow.infix,
    //          let.prefix, call.infix, if.prefix, arrow.infix
    assert_eq!(lang.grammar.len(), 14);
    // Eval: var, binary.add, binary.sub, binary.mul, binary.div, binary.pow,
    //       let, call, if, function (value has no scheme — no-scheme fallback)
    assert_eq!(lang.eval.len(), 10);
}

/// Verify the merged language is serializable (Language is pure data).
#[test]
fn full_language_serde_roundtrip() {
    let lang = full_language();
    let j = serde_json::to_string(&lang).unwrap();
    let back: Language = serde_json::from_str(&j).unwrap();
    assert_eq!(lang, back);
}
