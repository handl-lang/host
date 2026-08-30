//! # handl-plugin-engine-parse-pratt
//!
//! The **Pratt engine** for the parse operation (ADR-0044 §3).
//!
//! This engine reads [`handl_language::Language`] **grammar data** — it looks
//! up [`handl_language::GrammarRow`]s by `(token, position)` and drives a
//! classic top-down operator-precedence (Pratt) loop. It does **not** call
//! `rule.prefix(token, ctx)` on a Rust `ParserBehavior` object; it reads the
//! descriptor (prefix/infix, binding powers, child shape) from data and
//! executes.
//!
//! ## Root node convention
//!
//! The parse produces an [`Ast`] arena. The **root node is the last node
//! pushed** — in a Pratt parser, the outermost expression's node is always
//! the final one appended (children are pushed before their parents). For
//! empty input, the arena is empty and there is no root.
//!
//! ## Payload convention
//!
//! - **Atoms** (arity 0): the payload is the token's interpreted value — a
//!   number if the token kind id contains `"number"`, otherwise the token
//!   text as a string.
//! - **Compound nodes** (arity > 0): the payload is
//!   `{"children": [idx0, idx1, ...]}` where each `idxN` is the arena index
//!   (`AstRef::index()`) of the Nth child. The eval engine uses this to
//!   resolve [`handl_language::Scheme::Child`] references.

#![warn(missing_docs)]

use std::cell::RefCell;

use handl_language::{
    Ast, AstNode, AstRef, GrammarPosition, LangError, Language, Token, Value,
};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};

/// The Pratt parsing engine.
///
/// Reads `lang.grammar` data exclusively — no Rust `ParserBehavior` impls.
/// The engine owns an [`Ast`] arena via [`RefCell`] (reset at the start of
/// each [`ParserEngine::parse`] call) so that the recursive
/// [`ParserEngine::expression`] entry point can push nodes through `&self`.
///
/// Construct with [`PrattEngine::new`] and pass to any consumer of
/// `&dyn ParserEngine`.
pub struct PrattEngine {
    /// The arena being built during the current parse.
    ast: RefCell<Ast>,
}

impl PrattEngine {
    /// Create a new Pratt engine with an empty arena.
    pub fn new() -> Self {
        Self {
            ast: RefCell::new(Ast::new()),
        }
    }

    /// Interpret a token's text into a payload [`Value`], using the
    /// language's token kind definitions to decide how.
    ///
    /// If the token kind id contains `"number"`, the text is parsed as an
    /// integer (falling back to a float). If it contains `"bool"`, the text
    /// is parsed as a boolean (`"true"` → `true`, anything else → `false`).
    /// If it contains `"null"`, the payload is `Value::Null`. Otherwise the
    /// text is used as a string. This is data-driven: the engine reads
    /// `lang.tokens` to find the [`TokenKindDef`] for the token's kind.
    fn atom_payload(&self, lang: &Language, token: &Token) -> Value {
        let kind_def = lang.tokens.get_by_identity(&token.kind);
        let kind_id = kind_def.map(|tkd| tkd.id.as_str()).unwrap_or(&token.kind);

        if kind_id.contains("number") {
            if let Ok(n) = token.text.parse::<i64>() {
                return Value::from(n);
            }
            if let Ok(f) = token.text.parse::<f64>() {
                return Value::from(f);
            }
        }

        if kind_id.contains("bool") {
            return Value::from(token.text == "true");
        }

        if kind_id.contains("null") {
            return Value::Null;
        }

        // String tokens: the lexer includes the delimiters (quotes) in the
        // token text. Strip them to get the raw string value.
        if kind_id.contains("string") && token.text.len() >= 2 {
            let inner = &token.text[1..token.text.len() - 1];
            // Unescape basic escape sequences.
            let unescaped = inner
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
            return Value::from(unescaped);
        }

        Value::from(token.text.as_str())
    }

    /// Look up a grammar row by `(token_kind, position)` using the identity
    /// key `token@position`.
    fn lookup_row<'l>(
        lang: &'l Language,
        token_kind: &str,
        position: GrammarPosition,
    ) -> Option<&'l handl_language::GrammarRow> {
        let key = format!("{}@{}", token_kind, position.as_str());
        lang.grammar.get_by_identity(&key)
    }
}

impl Default for PrattEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserEngine for PrattEngine {
    fn parse(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
        // Reset the arena for this parse.
        *self.ast.borrow_mut() = Ast::new();

        let tokens = req.tokens.as_slice();
        if tokens.is_empty() {
            return Ok(Ast::new());
        }

        // Drive the Pratt loop from a local cursor. We do not use
        // ParserContext here because the engine owns the recursion; the
        // context is for external callers. We call expression directly.
        let mut cursor = Cursor {
            tokens,
            pos: 0,
        };
        let _root = self.expression_inner(lang, &mut cursor, 0)?;

        // Return the built arena. Root = last node pushed.
        Ok(self.ast.borrow().clone())
    }

    fn expression(
        &self,
        ctx: &mut handl_plugin_engine_parse::ParserContext<'_, '_, Self>,
        rbp: u8,
    ) -> Result<AstRef, LangError> {
        // Build a cursor from the context's remaining tokens.
        let tokens = ctx.tokens();
        let pos = ctx.pos();
        let mut cursor = Cursor { tokens, pos };
        let result = self.expression_inner(ctx.lang(), &mut cursor, rbp)?;
        // Sync the context position.
        // We need to advance the context to match the cursor's consumed
        // tokens. Since ParserContext::advance consumes one token, we call
        // it for each token the cursor consumed.
        let consumed = cursor.pos - pos;
        for _ in 0..consumed {
            let _ = ctx.advance()?;
        }
        Ok(result)
    }
}

/// A local cursor over a token slice, used internally by the Pratt loop.
struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Result<Token, LangError> {
        match self.tokens.get(self.pos) {
            Some(t) => {
                self.pos += 1;
                Ok(t.clone())
            }
            None => Err(LangError::Engine(
                "unexpected end of input".to_string(),
            )),
        }
    }
}

impl PrattEngine {
    /// The internal Pratt expression loop, driven entirely by `lang.grammar`
    /// data.
    fn expression_inner(
        &self,
        lang: &Language,
        cursor: &mut Cursor<'_>,
        rbp: u8,
    ) -> Result<AstRef, LangError> {
        let token = cursor.advance()?;

        // 1. Look up the prefix row for this token.
        let prefix_row = Self::lookup_row(lang, &token.kind, GrammarPosition::Prefix)
            .ok_or_else(|| LangError::Engine(format!("unexpected token: {}", token.kind)))?;

        let arity = prefix_row.child_shape.arity;
        let kind = prefix_row.child_shape.node_kind.clone();

        // 2. Build the prefix (null-denotation) node.
        let left = if arity == 0 {
            // Atom: payload is the token's interpreted value.
            let payload = self.atom_payload(lang, &token);
            let node = AstNode::new(&kind).with_payload(payload);
            self.ast.borrow_mut().push(node)
        } else {
            // Prefix operator: parse `arity` operands.
            let prefix_rbp = prefix_row
                .binding_power
                .map(|(l, _)| l)
                .unwrap_or(0);
            let mut children: Vec<usize> = Vec::with_capacity(arity);
            for _ in 0..arity {
                let child = self.expression_inner(lang, cursor, prefix_rbp)?;
                children.push(child.index());
            }
            let payload = serde_json::json!({ "children": children });
            let node = AstNode::new(&kind).with_payload(payload);
            self.ast.borrow_mut().push(node)
        };

        // 3. Infix (left-denotation) loop.
        let mut left = left;
        while let Some(next) = cursor.peek() {
            let infix_row = match Self::lookup_row(lang, &next.kind, GrammarPosition::Infix) {
                Some(row) => row,
                None => break,
            };

            let (lbp, rbp_right) = infix_row.binding_power.ok_or_else(|| {
                LangError::Engine(format!(
                    "infix row {} has no binding power",
                    infix_row.id
                ))
            })?;

            if lbp < rbp {
                break;
            }

            // Consume the operator token.
            let _op = cursor.advance()?;

            // Parse the right operand with the infix's right binding power.
            let right = self.expression_inner(lang, cursor, rbp_right)?;

            // Build the infix node: children = [left, right, ...].
            let infix_kind = infix_row.child_shape.node_kind.clone();
            let infix_arity = infix_row.child_shape.arity;
            let mut children: Vec<usize> = Vec::with_capacity(infix_arity);
            children.push(left.index());
            // For arity 2, add the right operand. For higher arity, parse more.
            children.push(right.index());
            for _ in 2..infix_arity {
                let extra = self.expression_inner(lang, cursor, rbp_right)?;
                children.push(extra.index());
            }
            let payload = serde_json::json!({ "children": children });
            let node = AstNode::new(&infix_kind).with_payload(payload);
            left = self.ast.borrow_mut().push(node);
        }

        Ok(left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        AstShapeRef, GrammarPosition, GrammarRow, TokenKindDef,
        Tokens,
    };
    use handl_plugin_engine_parse::{ParserContext, ParserRequest};
    use serde_json::json;

    /// Build a small arithmetic language: numbers, `+` (infix, lbp 10),
    /// `*` (infix, lbp 20), unary `-` (prefix).
    fn arith_lang() -> Language {
        let mut lang = Language::new();
        // Token kinds
        lang.tokens.merge(TokenKindDef::new("token.number")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.plus")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.star")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.minus")).unwrap();

        // Grammar: number prefix (atom, arity 0)
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            ))
            .unwrap();
        // Grammar: + infix (lbp 10, rbp 11)
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
        // Grammar: * infix (lbp 20, rbp 21)
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
        // Grammar: - prefix (arity 1, rbp 30)
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
        }
    }

    #[test]
    fn empty_input_returns_empty_ast() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens::new());
        let ast = engine.parse(&lang, &req).unwrap();
        assert!(ast.is_empty());
    }

    #[test]
    fn single_atom_parses_to_lit() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "42")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
        let root = &ast.nodes()[0];
        assert_eq!(root.kind, "lit");
        assert_eq!(root.payload, json!(42));
    }

    #[test]
    fn prefix_only_unary_neg() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "5"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 2);
        // Node 0: lit(5)
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[0].payload, json!(5));
        // Node 1: unary.neg(children=[0])
        assert_eq!(ast.nodes()[1].kind, "unary.neg");
        assert_eq!(ast.nodes()[1].payload, json!({"children": [0]}));
    }

    #[test]
    fn infix_only_binary_add() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 3);
        // Node 0: lit(1), Node 1: lit(2), Node 2: binary.add(children=[0,1])
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[1].kind, "lit");
        assert_eq!(ast.nodes()[2].kind, "binary.add");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
    }

    #[test]
    fn both_prefix_and_infix_negation_then_add() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 4);
        // Node 0: lit(1), Node 1: unary.neg([0]), Node 2: lit(2), Node 3: binary.add([1,2])
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[1].kind, "unary.neg");
        assert_eq!(ast.nodes()[2].kind, "lit");
        assert_eq!(ast.nodes()[3].kind, "binary.add");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
    }

    #[test]
    fn binding_power_precedence_mul_binds_tighter() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
            tok("token.star", "*"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Expected: 1 + (2 * 3)
        // Node 0: lit(1), Node 1: lit(2), Node 2: lit(3), Node 3: binary.mul([1,2]), Node 4: binary.add([0,3])
        assert_eq!(ast.len(), 5);
        assert_eq!(ast.nodes()[3].kind, "binary.mul");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
        assert_eq!(ast.nodes()[4].kind, "binary.add");
        assert_eq!(ast.nodes()[4].payload, json!({"children": [0, 3]}));
    }

    #[test]
    fn eof_handling_stops_infix_loop() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Should parse cleanly and stop at EOF.
        assert_eq!(ast.len(), 3);
    }

    #[test]
    fn unexpected_token_errors() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // token.bang has no prefix or infix rule.
        let req = ParserRequest::new(Tokens(vec![tok("token.bang", "!")]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
        let msg = format!("{}", err);
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn unexpected_token_after_partial_parse() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // 1 + ! — the "!" has no prefix rule.
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.bang", "!"),
        ]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn infix_without_binding_power_errors() {
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        // Add a broken infix row with no binding_power.
        lang.grammar
            .merge(GrammarRow::new(
                "g.broken",
                "token.q",
                GrammarPosition::Infix,
                AstShapeRef::new("binary.q", 2),
            ))
            .unwrap();
        lang.tokens.merge(TokenKindDef::new("token.q")).unwrap();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.q", "q"),
            tok("token.number", "2"),
        ]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
        let msg = format!("{}", err);
        assert!(msg.contains("binding power"));
    }

    #[test]
    fn atom_string_payload_for_non_number() {
        let engine = PrattEngine::new();
        let mut lang = Language::new();
        lang.tokens
            .merge(TokenKindDef::new("token.identifier"))
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.ident",
                "token.identifier",
                GrammarPosition::Prefix,
                AstShapeRef::new("identifier", 0),
            ))
            .unwrap();
        let req = ParserRequest::new(Tokens(vec![tok("token.identifier", "foo")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].kind, "identifier");
        assert_eq!(ast.nodes()[0].payload, json!("foo"));
    }

    #[test]
    fn atom_float_payload_for_float_number() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "3.14")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!(3.14));
    }

    #[test]
    fn root_is_last_node() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        let root_idx = ast.len() - 1;
        assert_eq!(ast.nodes()[root_idx].kind, "binary.add");
    }

    #[test]
    fn parser_context_expression_delegates() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let tokens = vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ];
        // Reset arena first.
        *engine.ast.borrow_mut() = Ast::new();
        let mut ctx = ParserContext::new(&engine, &lang, &tokens);
        let root = ctx.expression(0).unwrap();
        assert_eq!(root.index(), 2); // binary.add is node 2
        assert_eq!(ctx.pos(), 3); // consumed all 3 tokens
    }

    #[test]
    fn trait_expression_method_directly() {
        // Call the ParserEngine::expression trait method directly (not via
        // parse) to ensure the trait impl is covered.
        use handl_plugin_engine_parse::ParserEngine as _;
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let tokens = vec![
            tok("token.number", "5"),
            tok("token.star", "*"),
            tok("token.number", "3"),
        ];
        *engine.ast.borrow_mut() = Ast::new();
        let mut ctx = ParserContext::new(&engine, &lang, &tokens);
        // Call the trait method directly.
        let root = PrattEngine::expression(&engine, &mut ctx, 0).unwrap();
        // binary.mul is the last node pushed (index 2).
        assert_eq!(root.index(), 2);
        assert_eq!(ctx.pos(), 3);
    }

    #[test]
    fn atom_payload_token_kind_not_in_tokens_fragment_set() {
        // When a token kind has a grammar row but is NOT registered in
        // lang.tokens, atom_payload's get_by_identity returns None and
        // the unwrap_or_else fallback is used.
        let engine = PrattEngine::new();
        let mut lang = Language::new();
        // Register a grammar row for "token.foo" but do NOT register
        // "token.foo" in lang.tokens.
        lang.grammar
            .merge(GrammarRow::new(
                "g.foo",
                "token.foo",
                GrammarPosition::Prefix,
                AstShapeRef::new("foo", 0),
            ))
            .unwrap();
        let req = ParserRequest::new(Tokens(vec![tok("token.foo", "hello")]));
        let ast = engine.parse(&lang, &req).unwrap();
        // atom_payload falls back to checking token.kind.contains("number"),
        // which is false, so payload is the text "hello".
        assert_eq!(ast.nodes()[0].payload, json!("hello"));
    }

    #[test]
    fn trait_dispatch_via_generic() {
        fn run_parse<E: ParserEngine>(engine: &E, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
            engine.parse(lang, req)
        }
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "42")]));
        let ast = run_parse(&engine, &lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].payload, json!(42));
    }

    #[test]
    fn default_constructor() {
        let engine = PrattEngine::default();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "1")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
    }

    #[test]
    fn multiple_parses_reset_arena() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // First parse
        let req1 = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast1 = engine.parse(&lang, &req1).unwrap();
        assert_eq!(ast1.len(), 3);
        // Second parse should not contain nodes from the first.
        let req2 = ParserRequest::new(Tokens(vec![tok("token.number", "99")]));
        let ast2 = engine.parse(&lang, &req2).unwrap();
        assert_eq!(ast2.len(), 1);
        assert_eq!(ast2.nodes()[0].payload, json!(99));
    }

    #[test]
    fn left_associative_chain() {
        // 1 + 2 + 3 should parse as (1 + 2) + 3 (left-assoc).
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
            tok("token.plus", "+"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: lit(2), Node 2: binary.add([0,1]),
        // Node 3: lit(3), Node 4: binary.add([2,3])
        assert_eq!(ast.len(), 5);
        assert_eq!(ast.nodes()[2].kind, "binary.add");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
        assert_eq!(ast.nodes()[4].kind, "binary.add");
        assert_eq!(ast.nodes()[4].payload, json!({"children": [2, 3]}));
    }

    #[test]
    fn nested_prefix_in_infix() {
        // -1 + 2 → (-1) + 2
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: unary.neg([0]), Node 2: lit(2), Node 3: binary.add([1,2])
        assert_eq!(ast.len(), 4);
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
    }

    #[test]
    fn number_token_non_numeric_text_falls_back_to_string() {
        // A "token.number" with text that can't parse as number → string.
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "abc")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!("abc"));
    }

    #[test]
    fn prefix_op_at_eof_errors() {
        // Unary minus with no operand → cursor advance at EOF errors.
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.minus", "-")]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn next_token_without_infix_rule_stops() {
        // After parsing an atom, the next token exists but has no infix
        // rule → the infix loop breaks (None => break path).
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        // Add a token kind with a prefix rule but no infix rule.
        lang.tokens.merge(TokenKindDef::new("token.x")).unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.x",
                "token.x",
                GrammarPosition::Prefix,
                AstShapeRef::new("x", 0),
            ))
            .unwrap();
        // Tokens: number(1) x — after parsing number(1), the next token
        // is "x" which has no infix rule, so parsing stops.
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.x", "x"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Only the number atom is parsed; "x" is left unconsumed.
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "lit");
    }

    #[test]
    fn infix_arity_three_parses_extra_operands() {
        // An infix operator with arity 3 parses left + right + one extra.
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        lang.tokens.merge(TokenKindDef::new("token.ternary")).unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.ternary",
                    "token.ternary",
                    GrammarPosition::Infix,
                    AstShapeRef::new("ternary", 3),
                )
                .with_binding_power(5, 6),
            )
            .unwrap();
        // Tokens: 1 ? 2 3 — ternary infix with arity 3
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.ternary", "?"),
            tok("token.number", "2"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: lit(2), Node 2: lit(3)
        // Node 3: ternary {children: [0, 1, 2]}
        assert_eq!(ast.len(), 4);
        assert_eq!(ast.nodes()[3].kind, "ternary");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [0, 1, 2]}));
    }
}
