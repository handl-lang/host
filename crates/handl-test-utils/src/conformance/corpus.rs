//! Conformance corpus — `(Language, Ast, expected Value)` and
//! `(Language, Tokens, expected Ast)` triples as data.
//!
//! Seeded with simple cases covering `lit`, `add`, `let`, `if`, `foreign`,
//! and parse cases for atoms, infix, prefix, and precedence. Later phases
//! extend this corpus.

use handl_language::{
    Ast, AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Language, LiteralKind,
    Term, Token, TokenKindDef, Tokens, Value,
};

/// `arr(proj_child, lit: i)` — reference the i-th child node.
fn pc(i: u64) -> Term {
    Term::arr_native_lit("proj_child", vec![], Value::from(i))
}

/// An eval conformance case: a `(Language, Ast, expected Value)` triple.
#[derive(Debug, Clone)]
pub struct EvalCase {
    /// A human-readable name for the case.
    pub name: &'static str,
    /// The language definition (eval schemes, etc.).
    pub lang: Language,
    /// The AST to evaluate.
    pub ast: Ast,
    /// The expected result value.
    pub expected: Value,
}

/// A parse conformance case: a `(Language, Tokens, expected Ast)` triple.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseCase {
    /// A human-readable name for the case.
    pub name: &'static str,
    /// The language definition (grammar rows, token kinds, etc.).
    pub lang: Language,
    /// The token stream to parse.
    pub tokens: Tokens,
    /// The expected AST (compared by node kinds and payloads).
    pub expected: Ast,
}

/// Build the eval conformance corpus.
///
/// Returns a vector of [`EvalCase`]s covering: literal, add, let-binding, if,
/// foreign, and a nested let+add case. Later phases extend this.
pub fn eval_corpus() -> Vec<EvalCase> {
    vec![
        // 1. Literal: a "lit" node with no eval scheme → returns payload.
        EvalCase {
            name: "lit",
            lang: Language::new(),
            ast: {
                let mut a = Ast::new();
                a.push(AstNode::new("lit").with_payload(Value::from(42)));
                a
            },
            expected: Value::from(42),
        },
        // 2. Add: binary.add with arr("add", [proj_child 0, proj_child 1]).
        {
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.add",
                    "binary.add",
                    Term::arr_native("add", vec![pc(0), pc(1)]),
                ))
                .unwrap();
            EvalCase {
                name: "add",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("lit").with_payload(Value::from(2)));
                    a.push(AstNode::new("lit").with_payload(Value::from(3)));
                    a.push(
                        AstNode::new("binary.add")
                            .with_payload(serde_json::json!({"children": [0, 1]})),
                    );
                    a
                },
                expected: Value::from(5),
            }
        },
        // 3. Let: bind x = 42, body = var(x).
        {
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.var",
                    "var",
                    Term::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.let",
                    "let",
                    // value >>> arr(extend_env "$payload") >>> body
                    Term::seq(
                        Term::seq(
                            pc(0),
                            Term::arr_native_lit(
                                "extend_env",
                                vec![],
                                Value::String("$payload".into()),
                            ),
                        ),
                        pc(1),
                    ),
                ))
                .unwrap();
            EvalCase {
                name: "let",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("lit").with_payload(Value::from(42)));
                    a.push(AstNode::new("var").with_payload(Value::from("x")));
                    a.push(
                        AstNode::new("let")
                            .with_payload(serde_json::json!({"name": "x", "children": [0, 1]})),
                    );
                    a
                },
                expected: Value::from(42),
            }
        },
        // 4. If (true branch): cond >>> arr(tag) >>> Choice(then, else).
        {
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.if",
                    "if",
                    Term::seq(
                        Term::seq(pc(0), Term::arr_native("tag", vec![])),
                        Term::choice(pc(1), pc(2)),
                    ),
                ))
                .unwrap();
            EvalCase {
                name: "if_true",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("lit").with_payload(Value::Bool(true)));
                    a.push(AstNode::new("lit").with_payload(Value::from(1)));
                    a.push(AstNode::new("lit").with_payload(Value::from(2)));
                    a.push(
                        AstNode::new("if").with_payload(serde_json::json!({"children": [0, 1, 2]})),
                    );
                    a
                },
                expected: Value::from(1),
            }
        },
        // 5. Foreign: calls a host function "double".
        //    Note: the conformance harness provides a host registry with
        //    "double" registered. Engines without a host registry will error
        //    on this case; the harness handles that.
        {
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.dbl",
                    "dbl",
                    Term::arr_foreign("double", vec![pc(0)]),
                ))
                .unwrap();
            EvalCase {
                name: "foreign",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("lit").with_payload(Value::from(21)));
                    a.push(AstNode::new("dbl").with_payload(serde_json::json!({"children": [0]})));
                    a
                },
                expected: Value::from(42),
            }
        },
        // 6. Nested let + add: let x = 3 in let y = 4 in x + y.
        {
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.var",
                    "var",
                    Term::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.let",
                    "let",
                    // value >>> arr(extend_env "$payload") >>> body
                    Term::seq(
                        Term::seq(
                            pc(0),
                            Term::arr_native_lit(
                                "extend_env",
                                vec![],
                                Value::String("$payload".into()),
                            ),
                        ),
                        pc(1),
                    ),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.add",
                    "binary.add",
                    Term::arr_native("add", vec![pc(0), pc(1)]),
                ))
                .unwrap();
            EvalCase {
                name: "nested_let_add",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("lit").with_payload(Value::from(3)));
                    a.push(AstNode::new("lit").with_payload(Value::from(4)));
                    a.push(AstNode::new("var").with_payload(Value::from("x")));
                    a.push(AstNode::new("var").with_payload(Value::from("y")));
                    a.push(
                        AstNode::new("binary.add")
                            .with_payload(serde_json::json!({"children": [2, 3]})),
                    );
                    a.push(
                        AstNode::new("let")
                            .with_payload(serde_json::json!({"name": "y", "children": [1, 4]})),
                    );
                    a.push(
                        AstNode::new("let")
                            .with_payload(serde_json::json!({"name": "x", "children": [0, 5]})),
                    );
                    a
                },
                expected: Value::from(7),
            }
        },
        // Multi-shot `resume` (ADR-0158 workstream — replay journals):
        // `handle (perform(choose, [1,2]) + 100) with {choose: (opts,
        // resume) ~> [resume(opts[0]), resume(opts[1])]}`. The ≥2-ary
        // clause suspends the `perform` and each `resume` re-runs the body
        // with its argument fed back, so both continuations are collected.
        //
        // This is the cross-engine gate for the journal driver: tree-walk
        // (reference), bytecode (native replay) and cranelift (journal
        // via `handle_dyn` + the tree-walk arrow interpreter) must all
        // produce `[101, 102]`.
        {
            let child = |n: u64| Term::arr_native_lit("proj_child", vec![], Value::from(n));
            let prim = |op: &str, args: Vec<Term>| Term::arr_native(op, args);
            let mut lang = Language::new();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.var",
                    "var",
                    Term::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.add",
                    "binary.add",
                    prim("add", vec![child(0), child(1)]),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new("e.array", "array", prim("array", vec![])))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.index",
                    "idx",
                    prim("index", vec![child(0), child(1)]),
                ))
                .unwrap();
            // `call`: route the evaluated callee by type — string →
            // `call_host` (host fn, incl. `perform`), value → `App`
            // (arrows and `resume` objects).
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.call",
                    "call",
                    Term::seq(
                        Term::seq(
                            child(0),
                            Term::arr_native_lit("route_on_type", vec![], Value::String("string".into())),
                        ),
                        Term::choice(
                            prim("call_host", vec![prim("proj_rest", vec![])]),
                            Term::app(prim("id", vec![]), vec![prim("proj_rest", vec![])]),
                        ),
                    ),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.function",
                    "function",
                    prim("closure", vec![]),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.object",
                    "object",
                    prim("object", vec![]),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.handle",
                    "handle",
                    Term::handle(child(0), "", child(1)),
                ))
                .unwrap();
            EvalCase {
                name: "multishot_resume",
                lang,
                ast: {
                    let mut a = Ast::new();
                    a.push(AstNode::new("var").with_payload(Value::from("perform"))); // 0
                    a.push(AstNode::new("var").with_payload(Value::from("choose"))); // 1
                    a.push(AstNode::new("value").with_payload(Value::from(1))); // 2
                    a.push(AstNode::new("value").with_payload(Value::from(2))); // 3
                    a.push(AstNode::new("array").with_payload(serde_json::json!({"children": [2, 3]}))); // 4
                    a.push(AstNode::new("call").with_payload(serde_json::json!({"children": [0, 1, 4]}))); // 5
                    a.push(AstNode::new("value").with_payload(Value::from(100))); // 6
                    a.push(AstNode::new("binary.add").with_payload(serde_json::json!({"children": [5, 6]}))); // 7 body
                    a.push(AstNode::new("var").with_payload(Value::from("resume"))); // 8
                    a.push(AstNode::new("var").with_payload(Value::from("opts"))); // 9
                    a.push(AstNode::new("value").with_payload(Value::from(0))); // 10
                    a.push(AstNode::new("idx").with_payload(serde_json::json!({"children": [9, 10]}))); // 11
                    a.push(AstNode::new("call").with_payload(serde_json::json!({"children": [8, 11]}))); // 12
                    a.push(AstNode::new("value").with_payload(Value::from(1))); // 13
                    a.push(AstNode::new("idx").with_payload(serde_json::json!({"children": [9, 13]}))); // 14
                    a.push(AstNode::new("call").with_payload(serde_json::json!({"children": [8, 14]}))); // 15
                    a.push(AstNode::new("array").with_payload(serde_json::json!({"children": [12, 15]}))); // 16
                    a.push(AstNode::new("function").with_payload(serde_json::json!({"params": ["opts", "resume"], "body": 16}))); // 17
                    a.push(AstNode::new("key").with_payload(Value::from("choose"))); // 18
                    a.push(AstNode::new("entry").with_payload(serde_json::json!({"children": [18, 17]}))); // 19
                    a.push(AstNode::new("object").with_payload(serde_json::json!({"children": [19]}))); // 20
                    a.push(AstNode::new("handle").with_payload(serde_json::json!({"children": [7, 20]}))); // 21
                    a
                },
                expected: serde_json::json!([101, 102]),
            }
        },
    ]
}

/// Build the parse conformance corpus.
///
/// Returns a vector of [`ParseCase`]s covering: single atom, infix binary,
/// prefix unary, precedence, and empty input. Later phases extend this.
pub fn parse_corpus() -> Vec<ParseCase> {
    /// Build a small arithmetic language.
    fn arith_lang() -> Language {
        let mut lang = Language::new();
        lang.tokens
            .merge(TokenKindDef::new("token.number").with_literal(LiteralKind::Number))
            .unwrap();
        lang.tokens.merge(TokenKindDef::new("token.plus")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.star")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.minus")).unwrap();

        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            ))
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.plus",
                    "token.plus",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.add", 2),
                )
                .with_binding_power(10, 11),
            )
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.star",
                    "token.star",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.mul", 2),
                )
                .with_binding_power(20, 21),
            )
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.neg",
                    "token.minus",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("unary.neg", 1),
                )
                .with_binding_power(30, 0),
            )
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
            ..Default::default()
        }
    }

    vec![
        // 1. Single atom: 42 → lit(42)
        ParseCase {
            name: "single_atom",
            lang: arith_lang(),
            tokens: Tokens(vec![tok("token.number", "42")]),
            expected: {
                let mut a = Ast::new();
                a.push(AstNode::new("lit").with_payload(Value::from(42)));
                a
            },
        },
        // 2. Infix: 1 + 2 → binary.add([lit(1), lit(2)])
        ParseCase {
            name: "infix_add",
            lang: arith_lang(),
            tokens: Tokens(vec![
                tok("token.number", "1"),
                tok("token.plus", "+"),
                tok("token.number", "2"),
            ]),
            expected: {
                let mut a = Ast::new();
                a.push(AstNode::new("lit").with_payload(Value::from(1)));
                a.push(AstNode::new("lit").with_payload(Value::from(2)));
                a.push(
                    AstNode::new("binary.add")
                        .with_payload(serde_json::json!({"children": [0, 1]})),
                );
                a
            },
        },
        // 3. Prefix: -5 → unary.neg([lit(5)])
        ParseCase {
            name: "prefix_neg",
            lang: arith_lang(),
            tokens: Tokens(vec![tok("token.minus", "-"), tok("token.number", "5")]),
            expected: {
                let mut a = Ast::new();
                a.push(AstNode::new("lit").with_payload(Value::from(5)));
                a.push(
                    AstNode::new("unary.neg").with_payload(serde_json::json!({"children": [0]})),
                );
                a
            },
        },
        // 4. Precedence: 1 + 2 * 3 → binary.add([lit(1), binary.mul([lit(2), lit(3)])])
        ParseCase {
            name: "precedence",
            lang: arith_lang(),
            tokens: Tokens(vec![
                tok("token.number", "1"),
                tok("token.plus", "+"),
                tok("token.number", "2"),
                tok("token.star", "*"),
                tok("token.number", "3"),
            ]),
            expected: {
                let mut a = Ast::new();
                a.push(AstNode::new("lit").with_payload(Value::from(1)));
                a.push(AstNode::new("lit").with_payload(Value::from(2)));
                a.push(AstNode::new("lit").with_payload(Value::from(3)));
                a.push(
                    AstNode::new("binary.mul")
                        .with_payload(serde_json::json!({"children": [1, 2]})),
                );
                a.push(
                    AstNode::new("binary.add")
                        .with_payload(serde_json::json!({"children": [0, 3]})),
                );
                a
            },
        },
        // 5. Empty input → empty Ast
        ParseCase {
            name: "empty",
            lang: arith_lang(),
            tokens: Tokens::new(),
            expected: Ast::new(),
        },
    ]
}
