//! Conformance corpus — `(Language, Ast, expected Value)` and
//! `(Language, Tokens, expected Ast)` triples as data.
//!
//! Seeded with simple cases covering `lit`, `add`, `let`, `if`, `foreign`,
//! and parse cases for atoms, infix, prefix, and precedence. Later phases
//! extend this corpus.

use handl_language::{
    Ast, AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Language,
    LiteralKind, Scheme, Token, TokenKindDef, Tokens, Value,
};

/// `arr(proj_child, lit: i)` — reference the i-th child node.
fn pc(i: u64) -> Scheme {
    Scheme::arr_native_lit("proj_child", vec![], Value::from(i))
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
                    Scheme::arr_native("add", vec![pc(0), pc(1)]),
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
                    Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.let",
                    "let",
                    // value >>> arr(extend_env "$payload") >>> body
                    Scheme::seq(
                        Scheme::seq(
                            pc(0),
                            Scheme::arr_native_lit(
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
                    Scheme::seq(
                        Scheme::seq(pc(0), Scheme::arr_native("tag", vec![])),
                        Scheme::choice(pc(1), pc(2)),
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
                    Scheme::arr_foreign("double", vec![pc(0)]),
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
                    Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
                ))
                .unwrap();
            lang.eval
                .merge(EvalSchemeDef::new(
                    "e.let",
                    "let",
                    // value >>> arr(extend_env "$payload") >>> body
                    Scheme::seq(
                        Scheme::seq(
                            pc(0),
                            Scheme::arr_native_lit(
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
                    Scheme::arr_native("add", vec![pc(0), pc(1)]),
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
            .merge(
                TokenKindDef::new("token.number")
                    .with_literal(LiteralKind::Number),
            )
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
