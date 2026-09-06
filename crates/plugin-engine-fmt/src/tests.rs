use super::*;
use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, AstShapeRef, ChildRoleSpec, DelimitedShape, GrammarPosition,
    GrammarRow, Language, LiteralKind, PairShape, PayloadTokenSpec, TokenKindDef,
};
use serde_json::Value;

/// Build a minimal arithmetic language for formatter tests.
fn arith_lang() -> Language {
    let mut lang = Language::new();
    lang.tokens
        .merge(TokenKindDef::new("token.number").with_literal(LiteralKind::Number))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.identifier"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.plus").with_label("+"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.star").with_label("*"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.minus").with_label("-"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.lparen").with_label("("))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.rparen").with_label(")"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.lbrace").with_label("{"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.rbrace").with_label("}"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.semicolon").with_label(";"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.keyword.let").with_label("let"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.equals").with_label("="))
        .unwrap();

    // Grammar rows.
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            )
            .with_binding_power(0, 0),
        )
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
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.paren",
                "token.lparen",
                GrammarPosition::Prefix,
                AstShapeRef::new("paren", 1).with_delimited(DelimitedShape {
                    open: "token.lparen".into(),
                    close: "token.rparen".into(),
                    separator: None,
                    container_kind: "paren".into(),
                    pair: None,
                    open_is_operator_token: true,
                    flatten: true,
                    rest_token: None,
                }),
            )
            .with_binding_power(0, 0),
        )
        .unwrap();
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.let",
                "token.keyword.let",
                GrammarPosition::Prefix,
                AstShapeRef::new("let", 1).with_payload_tokens(vec![
                    PayloadTokenSpec {
                        token: "token.identifier".into(),
                        field: Some("name".into()),
                    },
                    PayloadTokenSpec {
                        token: "token.equals".into(),
                        field: None,
                    },
                ]),
            )
            .with_binding_power(0, 0),
        )
        .unwrap();
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.block",
                "token.lbrace",
                GrammarPosition::Prefix,
                AstShapeRef::new("block", 0).with_delimited(DelimitedShape {
                    open: "token.lbrace".into(),
                    close: "token.rbrace".into(),
                    separator: Some("token.semicolon".into()),
                    container_kind: "block".into(),
                    pair: None,
                    open_is_operator_token: true,
                    flatten: true,
                    rest_token: None,
                }),
            )
            .with_binding_power(0, 0),
        )
        .unwrap();

    // AST shapes.
    lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
    lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.binary.add", "binary.add").with_children(vec![
                ChildRoleSpec {
                    role: "lhs".into(),
                    variadic: false,
                },
                ChildRoleSpec {
                    role: "rhs".into(),
                    variadic: false,
                },
            ]),
        )
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.binary.mul", "binary.mul").with_children(vec![
                ChildRoleSpec {
                    role: "lhs".into(),
                    variadic: false,
                },
                ChildRoleSpec {
                    role: "rhs".into(),
                    variadic: false,
                },
            ]),
        )
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.unary.neg", "unary.neg").with_children(vec![ChildRoleSpec {
                role: "operand".into(),
                variadic: false,
            }]),
        )
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.let", "let").with_children(vec![ChildRoleSpec {
                role: "body".into(),
                variadic: false,
            }]),
        )
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.block", "block").with_children(vec![ChildRoleSpec {
                role: "statements".into(),
                variadic: true,
            }]),
        )
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.paren", "paren"))
        .unwrap();

    lang
}

fn lit(val: i64) -> AstNode {
    AstNode::new("lit").with_payload(Value::from(val))
}

fn var(name: &str) -> AstNode {
    AstNode::new("var").with_payload(serde_json::json!({"name": name}))
}

#[test]
fn format_single_literal() {
    let lang = arith_lang();
    let mut ast = Ast::new();
    let r = ast.push(lit(42));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "42");
}

#[test]
fn format_infix_add() {
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    let r =
        ast.push(AstNode::new("binary.add").with_payload(serde_json::json!({"children": [0, 1]})));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "1 + 2");
}

#[test]
fn format_precedence_no_extra_parens() {
    // 1 + 2 * 3 — mul binds tighter, no parens needed.
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    ast.push(lit(3));
    let mul =
        ast.push(AstNode::new("binary.mul").with_payload(serde_json::json!({"children": [1, 2]})));
    let add = ast.push(
        AstNode::new("binary.add").with_payload(serde_json::json!({"children": [0, mul.index()]})),
    );
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, add).unwrap();
    assert_eq!(out, "1 + 2 * 3");
}

#[test]
fn format_precedence_parens_for_grouping() {
    // (1 + 2) * 3 — the add is the lhs of mul; add's right bp (11) < mul's
    // left bp (20), so the left child needs parens.
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    ast.push(lit(3));
    let add =
        ast.push(AstNode::new("binary.add").with_payload(serde_json::json!({"children": [0, 1]})));
    let mul = ast.push(
        AstNode::new("binary.mul").with_payload(serde_json::json!({"children": [add.index(), 2]})),
    );
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, mul).unwrap();
    assert_eq!(out, "(1 + 2) * 3");
}

#[test]
fn format_unary_neg() {
    let lang = arith_lang();
    let mut ast = Ast::new();
    let v = ast.push(var("x"));
    let r = ast
        .push(AstNode::new("unary.neg").with_payload(serde_json::json!({"children": [v.index()]})));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "-x");
}

#[test]
fn format_leaf_fallback_from_payload() {
    // A `var` node has no grammar row; it should render from payload `name`.
    let lang = arith_lang();
    let mut ast = Ast::new();
    let r = ast.push(var("foo"));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "foo");
}

#[test]
fn format_let_with_payload_tokens() {
    // let x = 42
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(42));
    let r = ast
        .push(AstNode::new("let").with_payload(serde_json::json!({"name": "x", "children": [0]})));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "let x = 42");
}

#[test]
fn format_block_indentation() {
    // { 1; 2; 3 }
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    ast.push(lit(3));
    let r =
        ast.push(AstNode::new("block").with_payload(serde_json::json!({"children": [0, 1, 2]})));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "{\n  1;\n  2;\n  3\n}");
}

#[test]
fn format_nested_block_indentation() {
    // { { 1; 2 }; 3 }
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    let inner =
        ast.push(AstNode::new("block").with_payload(serde_json::json!({"children": [0, 1]})));
    ast.push(lit(3));
    let r = ast.push(
        AstNode::new("block").with_payload(serde_json::json!({"children": [inner.index(), 3]})),
    );
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "{\n  {\n    1;\n    2\n  };\n  3\n}");
}

#[test]
fn format_unknown_kind_fallback() {
    // A node kind with no grammar row and no recognizable payload falls back
    // to rendering the kind itself as a placeholder.
    let lang = arith_lang();
    let mut ast = Ast::new();
    let r = ast.push(AstNode::new("mystery"));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "mystery");
}

#[test]
fn format_paren_grouping() {
    // paren wrapping a single expression: ( 1 + 2 )
    // The paren node uses a delimited shape with flatten=true.
    let lang = arith_lang();
    let mut ast = Ast::new();
    ast.push(lit(1));
    ast.push(lit(2));
    let add =
        ast.push(AstNode::new("binary.add").with_payload(serde_json::json!({"children": [0, 1]})));
    let r = ast
        .push(AstNode::new("paren").with_payload(serde_json::json!({"children": [add.index()]})));
    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "( 1 + 2 )");
}

#[test]
fn format_match_arms_with_pairs() {
    // Build a language with a match construct using pair-shaped arms.
    let mut lang = arith_lang();
    lang.tokens
        .merge(TokenKindDef::new("token.keyword.match").with_label("match"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.colon").with_label(":"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.comma").with_label(","))
        .unwrap();
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.match",
                "token.keyword.match",
                GrammarPosition::Prefix,
                AstShapeRef::new("match", 2).with_delimited(DelimitedShape {
                    open: "token.lbrace".into(),
                    close: "token.rbrace".into(),
                    separator: Some("token.comma".into()),
                    container_kind: "block".into(),
                    pair: Some(PairShape {
                        separator: "token.colon".into(),
                        node_kind: "arm".into(),
                    }),
                    open_is_operator_token: false,
                    flatten: false,
                    rest_token: None,
                }),
            )
            .with_binding_power(0, 0),
        )
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.match", "match").with_children(vec![
            ChildRoleSpec {
                role: "scrutinee".into(),
                variadic: false,
            },
            ChildRoleSpec {
                role: "arms".into(),
                variadic: false,
            },
        ]))
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.arm", "arm").with_children(vec![
            ChildRoleSpec {
                role: "pattern".into(),
                variadic: false,
            },
            ChildRoleSpec {
                role: "body".into(),
                variadic: false,
            },
        ]))
        .unwrap();

    // match x { 1 : 2, 3 : 4 }
    let mut ast = Ast::new();
    let scrut = ast.push(var("x"));
    // Arm 1: pattern=lit(1), body=lit(2)
    let p1 = ast.push(lit(1));
    let b1 = ast.push(lit(2));
    let arm1 = ast.push(
        AstNode::new("arm").with_payload(serde_json::json!({"children": [p1.index(), b1.index()]})),
    );
    // Arm 2: pattern=lit(3), body=lit(4)
    let p2 = ast.push(lit(3));
    let b2 = ast.push(lit(4));
    let arm2 = ast.push(
        AstNode::new("arm").with_payload(serde_json::json!({"children": [p2.index(), b2.index()]})),
    );
    // Block container holding the arms.
    let block = ast.push(
        AstNode::new("block")
            .with_payload(serde_json::json!({"children": [arm1.index(), arm2.index()]})),
    );
    let r = ast.push(
        AstNode::new("match")
            .with_payload(serde_json::json!({"children": [scrut.index(), block.index()]})),
    );

    let engine = FormatEngine::new();
    let out = engine.format(&lang, &ast, r).unwrap();
    assert_eq!(out, "match x {\n  1 : 2,\n  3 : 4\n}");
}
