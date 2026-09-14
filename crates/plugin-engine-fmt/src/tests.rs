use super::*;
use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, AstShapeRef, ChildRoleSpec, DelimitedShape, GrammarPosition,
    GrammarRow, Language, LiteralKind, PairShape, PayloadTokenSpec, Span, Token, TokenKindDef,
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

#[test]
fn preserve_unchanged_round_trip() {
    // 1 + 2 — the whole add node is unchanged and carries the original span,
    // so it should splice back exactly.
    let lang = arith_lang();
    let mut ast = Ast::new();
    let one = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(1))
            .with_span(Span { start: 0, end: 1 }),
    );
    let two = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(2))
            .with_span(Span { start: 4, end: 5 }),
    );
    let add = ast.push(
        AstNode::new("binary.add")
            .with_payload(serde_json::json!({"children": [one.index(), two.index()]}))
            .with_span(Span { start: 0, end: 5 }),
    );
    let src = "1 + 2";
    let emitter = PreserveEmitter::new(src, &ast, add, &[]).unwrap();
    let out = emitter.format(&lang, &ast, add).unwrap();
    assert_eq!(out, "1 + 2");
}

#[test]
fn preserve_changed_child_reformats_only_that_region() {
    // Change the rhs literal to 99; only that child should reformat, the lhs
    // is spliced from the original span.
    let lang = arith_lang();
    let mut ast = Ast::new();
    let one = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(1))
            .with_span(Span { start: 0, end: 1 }),
    );
    let two = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(99))
            .with_span(Span { start: 4, end: 5 }),
    );
    let add = ast.push(
        AstNode::new("binary.add")
            .with_payload(serde_json::json!({"children": [one.index(), two.index()]}))
            .with_span(Span { start: 0, end: 5 }),
    );
    let src = "1 + 2";
    let changed: Vec<AstRef> = vec![two];
    let emitter = PreserveEmitter::new(src, &ast, add, &changed).unwrap();
    let out = emitter.format(&lang, &ast, add).unwrap();
    assert_eq!(out, "1 + 99");
}

#[test]
fn preserve_comments_in_unchanged_subtree() {
    // Unchanged let node with a trailing comment should be spliced verbatim.
    let lang = arith_lang();
    let src = "let x = 1 + 2 -- keep";
    let mut ast = Ast::new();
    let one = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(1))
            .with_span(Span { start: 8, end: 9 }),
    );
    let two = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(2))
            .with_span(Span { start: 12, end: 13 }),
    );
    let add = ast.push(
        AstNode::new("binary.add")
            .with_payload(serde_json::json!({"children": [one.index(), two.index()]}))
            .with_span(Span { start: 8, end: 13 }),
    );
    let let_node = ast.push(
        AstNode::new("let")
            .with_payload(serde_json::json!({"name": "x", "children": [add.index()]}))
            .with_span(Span { start: 0, end: 21 }),
    );
    let emitter = PreserveEmitter::new(src, &ast, let_node, &[]).unwrap();
    let out = emitter.format(&lang, &ast, let_node).unwrap();
    assert_eq!(out, "let x = 1 + 2 -- keep");
}

#[test]
fn preserve_nested_change_inside_unchanged_block() {
    // First let is spliced verbatim; second let was rewritten and its new
    // multiply node (span: None) is reformatted, while its literal children
    // are still spliced from the original source.
    let lang = arith_lang();
    let src = "{let x = 1 + 2;let y = 3 + 4}";
    let mut ast = Ast::new();
    let one = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(1))
            .with_span(Span { start: 9, end: 10 }),
    );
    let two = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(2))
            .with_span(Span { start: 13, end: 14 }),
    );
    let add1 = ast.push(
        AstNode::new("binary.add")
            .with_payload(serde_json::json!({"children": [one.index(), two.index()]}))
            .with_span(Span { start: 9, end: 14 }),
    );
    let let1 = ast.push(
        AstNode::new("let")
            .with_payload(serde_json::json!({"name": "x", "children": [add1.index()]}))
            .with_span(Span { start: 1, end: 14 }),
    );
    let three = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(3))
            .with_span(Span { start: 23, end: 24 }),
    );
    let four = ast.push(
        AstNode::new("lit")
            .with_payload(Value::from(4))
            .with_span(Span { start: 27, end: 28 }),
    );
    let mul = ast.push(
        AstNode::new("binary.mul")
            .with_payload(serde_json::json!({"children": [three.index(), four.index()]})),
    );
    let let2 = ast.push(
        AstNode::new("let")
            .with_payload(serde_json::json!({"name": "y", "children": [mul.index()]}))
            .with_span(Span { start: 15, end: 28 }),
    );
    let block = ast.push(
        AstNode::new("block")
            .with_payload(serde_json::json!({"children": [let1.index(), let2.index()]}))
            .with_span(Span { start: 0, end: 29 }),
    );
    let changed: Vec<AstRef> = vec![let2];
    let emitter = PreserveEmitter::new(src, &ast, block, &changed).unwrap();
    let out = emitter.format(&lang, &ast, block).unwrap();
    assert_eq!(out, "{\n  let x = 1 + 2;\n  let y = 3 * 4\n}");
}
// ---------------------------------------------------------------------------
// ADR-0095 pipeline: layout (stage 2), render (stage 3), post-process (stage 4)
// ---------------------------------------------------------------------------

/// A categorized language covering the [`TokenCat`] classifications.
fn categorized_lang() -> Language {
    let mut lang = Language::new();
    lang.tokens
        .merge(
            TokenKindDef::new("token.lparen")
                .with_category("open-bracket")
                .with_label("("),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.lbrace")
                .with_category("open-bracket")
                .with_label("{"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.rparen")
                .with_category("close-bracket")
                .with_label(")"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.rbrace")
                .with_category("close-bracket")
                .with_label("}"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.rbracket")
                .with_category("close-bracket")
                .with_label("]"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.comma")
                .with_category("punctuation")
                .with_label(","),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.colon")
                .with_category("punctuation")
                .with_label(":"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.dot")
                .with_category("punctuation")
                .with_label("."),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.plus")
                .with_category("operator")
                .with_label("+"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.bang")
                .with_category("operator")
                .with_label("!"),
        )
        .unwrap();
    lang.tokens
        .merge(
            TokenKindDef::new("token.keyword.if")
                .with_category("keyword")
                .with_label("if"),
        )
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.identifier").with_category("identifier"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.number").with_category("number"))
        .unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.comment").with_category("comment"))
        .unwrap();
    lang
}

#[test]
fn classify_token_by_category() {
    let lang = categorized_lang();
    assert_eq!(classify_token(&lang, "token.lparen"), TokenCat::OpenParen);
    assert_eq!(classify_token(&lang, "token.lbrace"), TokenCat::OpenBrace);
    assert_eq!(classify_token(&lang, "token.rparen"), TokenCat::CloseParen);
    assert_eq!(classify_token(&lang, "token.rbrace"), TokenCat::CloseBrace);
    assert_eq!(
        classify_token(&lang, "token.rbracket"),
        TokenCat::CloseBracket
    );
    assert_eq!(classify_token(&lang, "token.comma"), TokenCat::Comma);
    assert_eq!(classify_token(&lang, "token.colon"), TokenCat::Colon);
    assert_eq!(classify_token(&lang, "token.dot"), TokenCat::Dot);
    assert_eq!(classify_token(&lang, "token.plus"), TokenCat::BinaryOp);
    // `!` is an operator with a non-infix spelling → Other.
    assert_eq!(classify_token(&lang, "token.bang"), TokenCat::Other);
    assert_eq!(classify_token(&lang, "token.keyword.if"), TokenCat::Keyword);
    assert_eq!(classify_token(&lang, "token.number"), TokenCat::Atom);
    assert_eq!(classify_token(&lang, "token.identifier"), TokenCat::Atom);
    assert_eq!(classify_token(&lang, "token.comment"), TokenCat::Comment);
    // Unknown kind → Other.
    assert_eq!(classify_token(&lang, "token.unknown"), TokenCat::Other);
}

#[test]
fn canonical_formatting_rule_and_needs_space() {
    use TokenCat::*;
    let rule = CanonicalFormattingRule;
    assert_eq!(rule.name(), "canonical-spacing");
    assert!(rule.needs_space(Atom, BinaryOp));
    assert!(rule.needs_space(BinaryOp, Atom));
    assert!(needs_space(Atom, BinaryOp));
    assert!(!needs_space(OpenBracket, Atom));
    assert!(!needs_space(Atom, CloseParen));
    assert!(!needs_space(Atom, Comma));
    assert!(!needs_space(Dot, Atom));
    assert!(!needs_space(OpenBrace, CloseBrace)); // empty {}
    assert!(needs_space(Keyword, Atom));
}

#[test]
fn indent_bias_classifies_brackets() {
    use TokenCat::*;
    assert_eq!(indent_bias(OpenParen), IndentBias::Open);
    assert_eq!(indent_bias(OpenBracket), IndentBias::Open);
    assert_eq!(indent_bias(OpenBrace), IndentBias::Open);
    assert_eq!(indent_bias(CloseParen), IndentBias::Close);
    assert_eq!(indent_bias(CloseBracket), IndentBias::Close);
    assert_eq!(indent_bias(CloseBrace), IndentBias::Close);
    assert_eq!(indent_bias(Atom), IndentBias::None);
}

#[test]
fn format_options_default_has_indent_and_width() {
    let o = FormatOptions::default();
    assert_eq!(o.indent_size, 2);
    assert_eq!(o.width, 100);
}
#[test]
fn token_formatter_spacing_and_custom_rule() {
    let lang = categorized_lang();
    let tokens: Vec<Token> = vec![
        Token {
            kind: "token.identifier".into(),
            text: "x".into(),
            ..Default::default()
        },
        Token {
            kind: "token.plus".into(),
            text: "+".into(),
            ..Default::default()
        },
        Token {
            kind: "token.number".into(),
            text: "2".into(),
            ..Default::default()
        },
    ];
    // Canonical spacer → `x + 2`.
    let canonical = TokenFormatter::canonical(2);
    assert_eq!(canonical.format("x + 2", &tokens, &lang), "x + 2");
    // A replaced rule (the ADR-0095 seam) changes output without touching the
    // render loop: a no-space rule joins tokens tightly.
    struct NoSpace;
    impl FormattingRule for NoSpace {
        fn name(&self) -> &str {
            "no-space"
        }

        fn needs_space(&self, _prev: TokenCat, _curr: TokenCat) -> bool {
            false
        }
    }
    let tight = TokenFormatter::new(Box::new(NoSpace), 2);
    assert_eq!(tight.format("x + 2", &tokens, &lang), "x+2");
}

#[test]
fn token_formatter_preserves_source_leading_ws() {
    let lang = categorized_lang();
    // A newline-gapped token should keep the original leading whitespace.
    let tokens: Vec<Token> = vec![
        Token {
            kind: "token.lbrace".into(),
            text: "{".into(),
            start: 0,
            end: 1,
        },
        Token {
            kind: "token.identifier".into(),
            text: "x".into(),
            start: 4,
            end: 5,
        },
        Token {
            kind: "token.rbrace".into(),
            text: "}".into(),
            start: 6,
            end: 7,
        },
    ];
    let formatter = TokenFormatter::canonical(2);
    let out = formatter.format("{\n  x\n}", &tokens, &lang);
    assert_eq!(out, "{\n  x\n}");
}

#[test]
fn post_process_individual_rules() {
    let rule = TrailingWhitespaceRule;
    assert_eq!(rule.apply("a   \nb \n"), "a\nb");
    let collapse = CollapseBlankLinesRule;
    assert_eq!(collapse.apply("a\n\n\nb"), "a\n\nb");
    let groups = BlankLineBetweenDeclGroupsRule;
    assert_eq!(
        groups.apply("import a from \"x\"\nmain :: f"),
        "import a from \"x\"\n\nmain :: f"
    );
    assert_eq!(
        groups.apply("import a from \"x\"\nimport b from \"y\""),
        "import a from \"x\"\nimport b from \"y\""
    );
    let newline = SingleTrailingNewlineRule;
    assert_eq!(newline.apply("x\n\n"), "x\n");
    assert_eq!(newline.apply(""), "");
    let canon_ops = CanonicalizeOperatorsRule;
    assert_eq!(canon_ops.apply("x : = y"), "x := y");
    assert_eq!(canon_ops.apply("export at < T >: T ~ [Total] ~> T"), "export at<T>: T ~[Total]~> T");
    assert_eq!(canon_ops.apply("export zip < A, B >: T ~ [Total] ~> T"), "export zip<A, B>: T ~[Total]~> T");
    assert_eq!(canon_ops.apply("export v4: () - > String"), "export v4: () -> String");
    // A `-` (minus) directly followed by `>` with no space is left alone.
    assert_eq!(canon_ops.apply("a->b"), "a->b");
}

#[test]
fn canonical_post_process_full_chain() {
    let out = canonical_post_process("import sort from \"c\"   \nmain :: f([1])   \n\n");
    assert_eq!(out, "import sort from \"c\"\n\nmain :: f([1])\n");
    assert_eq!(canonical_post_process(""), "");
}
