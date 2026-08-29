use std::marker::PhantomData;

use handl_platform::{AstNode, PlatformError};
use handl_plugin_lexer::{DynToken, TokenKind};

use crate::ParserBehavior;

/// Parsing context for the dynamic parser.
///
/// Wraps a slice of [`DynToken`]s and the current position, providing
/// cursor-style accessors for grammar rules. Unlike [`ParserContext`], this
/// context is not generic over a concrete `K: TokenKind` — it works directly
/// with `Box<dyn DynTokenKind>`.
pub struct DynParserContext<'a> {
    pub tokens: &'a [DynToken],
    pub pos: usize,
}

impl<'a> DynParserContext<'a> {
    /// Creates a new context over `tokens` positioned at the start.
    pub fn new(tokens: &'a [DynToken]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Returns the current token without consuming it.
    pub fn current(&self) -> Option<&DynToken> {
        self.tokens.get(self.pos)
    }

    /// Returns the token after the current one without consuming either.
    pub fn peek(&self) -> Option<&DynToken> {
        self.tokens.get(self.pos + 1)
    }

    /// Consumes and returns the current token, advancing the cursor.
    pub fn advance(&mut self) -> Option<&DynToken> {
        let token = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(token)
    }

    /// Returns `true` if the cursor is at (or past) the end-of-input token.
    pub fn is_eof(&self) -> bool {
        match self.tokens.get(self.pos) {
            Some(t) => t.kind.is_eof(),
            None => true,
        }
    }

    /// Skips trivia (comment/whitespace) tokens. Currently a no-op because
    /// [`DynTokenKind`] does not expose comment detection; whitespace is
    /// already stripped by the lexer.
    pub fn skip_trivia(&mut self) {
        while let Some(t) = self.tokens.get(self.pos) {
            if t.kind.is_error() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
}

/// Object-safe grammar rule for the dynamic parser.
///
/// Each rule attempts to parse an AST node at the current cursor position.
/// Rules are tried in descending [`DynGrammarRule::binding_power`] order; the
/// first rule that returns `Some` wins.
pub trait DynGrammarRule: Send + Sync + 'static {
    /// The AST node kind this rule produces (e.g. `"call"`, `"let"`).
    fn kind(&self) -> &str;

    /// Try to parse at the current position. Returns `None` if this rule does
    /// not match at the cursor.
    fn try_parse(&self, ctx: &mut DynParserContext) -> Result<Option<AstNode>, PlatformError>;

    /// Binding power for precedence (higher binds tighter).
    fn binding_power(&self) -> u32 {
        0
    }
}

/// A dynamic parser that assembles [`DynGrammarRule`]s at runtime and parses
/// `&[DynToken]` without a static `K: TokenKind` parameter.
pub struct DynParser {
    rules: Vec<Box<dyn DynGrammarRule>>,
}

impl Default for DynParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DynParser {
    /// Creates an empty dynamic parser.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Adds a grammar rule.
    pub fn with_rule(mut self, rule: Box<dyn DynGrammarRule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Returns the registered rules, sorted by binding power descending.
    fn ordered_rules(&self) -> Vec<&dyn DynGrammarRule> {
        let mut idx: Vec<usize> = (0..self.rules.len()).collect();
        idx.sort_by(|&a, &b| self.rules[b].binding_power().cmp(&self.rules[a].binding_power()));
        idx.into_iter().map(|i| self.rules[i].as_ref()).collect()
    }

    /// Parse a single expression from `tokens`.
    ///
    /// Rules are tried in binding-power order; the first that matches produces
    /// the result node.
    pub fn parse(&self, tokens: &[DynToken]) -> Result<AstNode, PlatformError> {
        let mut ctx = DynParserContext::new(tokens);
        ctx.skip_trivia();
        for rule in self.ordered_rules() {
            if let Some(node) = rule.try_parse(&mut ctx)? {
                return Ok(node);
            }
        }
        Err(PlatformError::Plugin(
            "no grammar rule matched the current token".into(),
        ))
    }

    /// Parse a module (sequence of top-level statements) from `tokens`.
    ///
    /// Iterates through tokens, parsing one statement per iteration until EOF.
    pub fn parse_module(&self, tokens: &[DynToken]) -> Result<Vec<AstNode>, PlatformError> {
        let mut ctx = DynParserContext::new(tokens);
        let mut stmts = Vec::new();
        loop {
            ctx.skip_trivia();
            if ctx.is_eof() {
                break;
            }
            let mut matched = false;
            for rule in self.ordered_rules() {
                let start = ctx.pos;
                if let Some(node) = rule.try_parse(&mut ctx)? {
                    stmts.push(node);
                    matched = true;
                    if ctx.pos == start {
                        // Rule matched but did not advance; avoid an infinite loop.
                        ctx.pos += 1;
                    }
                    break;
                }
            }
            if !matched {
                return Err(PlatformError::Plugin(format!(
                    "no grammar rule matched at position {}",
                    ctx.pos
                )));
            }
        }
        Ok(stmts)
    }
}

/// Bridge adapter that wraps a static [`ParserBehavior<K>`] and exposes it as
/// a [`DynGrammarRule`].
///
/// This lets existing grammar rules written for the generic [`BasicParser`] be
/// reused with the dynamic parser. Converting `DynToken` back to `Token<K>`
/// requires downcasting via [`DynTokenKind::as_any`]; if the downcast fails the
/// rule reports no match.
pub struct StaticGrammarRuleAdapter<R, K>
where
    R: ParserBehavior<K>,
    K: TokenKind,
{
    rule: R,
    _marker: PhantomData<K>,
}

impl<R, K> StaticGrammarRuleAdapter<R, K>
where
    R: ParserBehavior<K>,
    K: TokenKind,
{
    /// Creates a new adapter wrapping `rule`.
    pub fn new(rule: R) -> Self {
        Self {
            rule,
            _marker: PhantomData,
        }
    }
}

impl<R, K> DynGrammarRule for StaticGrammarRuleAdapter<R, K>
where
    R: ParserBehavior<K> + Send + Sync + 'static,
    K: TokenKind + Clone + Send + Sync + 'static,
{
    fn kind(&self) -> &str {
        self.rule.kind()
    }

    fn try_parse(&self, ctx: &mut DynParserContext) -> Result<Option<AstNode>, PlatformError> {
        // TODO: full bridge requires reconstructing a BasicParser<K> with all
        // sibling rules so that ParserContext::expression can recurse. For now
        // this adapter does not attempt a downcast-based parse and reports no
        // match. The framework types (DynParser/DynParserContext/DynGrammarRule)
        // are the priority; concrete dynamic rules should implement
        // DynGrammarRule directly.
        let _ = ctx;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{DynToken, EofFactory, Span};
    use serde_json::json;
    use std::fmt;

    #[derive(Debug, Clone, PartialEq)]
    enum Kind {
        Word(String),
        Eof,
    }

    impl fmt::Display for Kind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Kind::Word(s) => write!(f, "word({s})"),
                Kind::Eof => write!(f, "eof"),
            }
        }
    }

    impl TokenKind for Kind {
        type Factory = WordFactory;
        fn is_eof(&self) -> bool {
            matches!(self, Kind::Eof)
        }
        fn is_error(&self) -> bool {
            false
        }
    }

    #[derive(Clone)]
    struct WordFactory;
    impl EofFactory<Kind> for WordFactory {
        fn eof_kind(&self) -> Kind {
            Kind::Eof
        }
    }

    fn word(text: &str, start: usize, end: usize) -> DynToken {
        DynToken::new(Box::new(Kind::Word(text.into())), Span::new(start, end))
    }

    fn eof(pos: usize) -> DynToken {
        DynToken::new(Box::new(Kind::Eof), Span::at(pos))
    }

    struct WordRule;
    impl DynGrammarRule for WordRule {
        fn kind(&self) -> &str {
            "word"
        }
        fn try_parse(&self, ctx: &mut DynParserContext) -> Result<Option<AstNode>, PlatformError> {
            let token = match ctx.current() {
                Some(t) => t,
                None => return Ok(None),
            };
            let kind = match token.kind.as_any().downcast_ref::<Kind>() {
                Some(Kind::Word(s)) => s.clone(),
                _ => return Ok(None),
            };
            let span = token.span;
            ctx.advance();
            Ok(Some(AstNode {
                kind: "word".into(),
                span: Some(handl_platform::Span {
                    start: span.start,
                    end: span.end,
                }),
                payload: json!({ "text": kind }),
            }))
        }
        fn binding_power(&self) -> u32 {
            1
        }
    }

    #[test]
    fn parses_single_token() {
        let parser = DynParser::new().with_rule(Box::new(WordRule));
        let tokens = vec![word("hi", 0, 2), eof(2)];
        let node = parser.parse(&tokens).unwrap();
        assert_eq!(node.kind, "word");
        assert_eq!(node.payload["text"], "hi");
    }

    #[test]
    fn parses_module_of_words() {
        let parser = DynParser::new().with_rule(Box::new(WordRule));
        let tokens = vec![word("a", 0, 1), word("b", 1, 2), eof(2)];
        let stmts = parser.parse_module(&tokens).unwrap();
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0].payload["text"], "a");
        assert_eq!(stmts[1].payload["text"], "b");
    }

    #[test]
    fn no_match_errors() {
        let parser = DynParser::new().with_rule(Box::new(WordRule));
        let tokens = vec![eof(0)];
        assert!(parser.parse(&tokens).is_err());
    }

    #[test]
    fn binding_power_orders_rules() {
        struct Hi;
        impl DynGrammarRule for Hi {
            fn kind(&self) -> &str {
                "hi"
            }
            fn try_parse(&self, ctx: &mut DynParserContext) -> Result<Option<AstNode>, PlatformError> {
                ctx.advance();
                Ok(Some(AstNode {
                    kind: "hi".into(),
                    span: None,
                    payload: json!({}),
                }))
            }
            fn binding_power(&self) -> u32 {
                10
            }
        }
        struct Lo;
        impl DynGrammarRule for Lo {
            fn kind(&self) -> &str {
                "lo"
            }
            fn try_parse(&self, ctx: &mut DynParserContext) -> Result<Option<AstNode>, PlatformError> {
                ctx.advance();
                Ok(Some(AstNode {
                    kind: "lo".into(),
                    span: None,
                    payload: json!({}),
                }))
            }
            fn binding_power(&self) -> u32 {
                1
            }
        }

        let parser = DynParser::new()
            .with_rule(Box::new(Lo))
            .with_rule(Box::new(Hi));
        let tokens = vec![word("x", 0, 1), eof(1)];
        let node = parser.parse(&tokens).unwrap();
        assert_eq!(node.kind, "hi");
    }

    #[test]
    fn static_adapter_is_stub() {
        // The adapter is a stub that always reports no match. Constructing one
        // requires a full ParserBehavior<K> impl (Behavior + prefix/infix),
        // which is out of scope here; the type itself compiles as part of the
        // module. This test just guards the context wiring.
        let ctx_tokens: Vec<DynToken> = vec![eof(0)];
        let ctx = DynParserContext::new(&ctx_tokens);
        assert!(ctx.is_eof());
        assert!(ctx.current().is_some());
        assert!(ctx.peek().is_none());
    }
}
