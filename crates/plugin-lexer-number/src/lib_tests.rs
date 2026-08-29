use super::*;
use handl_plugin_lexer::{DefaultLexError, LexRule, Span, TokenKind};
use handl_plugin_lexer_common::{MockFactory, MockKind, empty_config as cfg};

// Orphan-rule-allowed impl: NumberToken is local to this crate.
impl NumberToken<MockKind> for MockFactory {
    fn is_number_start(&self, ch: char) -> bool {
        ch.is_ascii_digit() || ch == '-'
    }

    fn number_kind(&self, value: f64) -> MockKind {
        (self.number_kind_fn())(value)
    }
}

const RULE: NumberRule = NumberRule;

#[test]
fn number_rule_matches_integer() {
    let config = cfg();
    let mut chars = "42 rest".char_indices().peekable();
    let result = NumberRule.try_lex(&mut chars, &config);
    let token = result.unwrap().unwrap();
    assert_eq!(token.kind, MockKind::Num(42.0));
    assert_eq!(token.span, Span::new(0, 2));
}

#[test]
fn number_rule_matches_float() {
    let config = cfg();
    let mut chars = "1.5 rest".char_indices().peekable();
    let result = NumberRule.try_lex(&mut chars, &config);
    let token = result.unwrap().unwrap();
    assert_eq!(token.kind, MockKind::Num(1.5));
}

#[test]
fn number_rule_matches_negative() {
    let config = cfg();
    let mut chars = "-3 rest".char_indices().peekable();
    let result = NumberRule.try_lex(&mut chars, &config);
    let token = result.unwrap().unwrap();
    assert_eq!(token.kind, MockKind::Num(-3.0));
}

#[test]
fn number_rule_dash_without_digit_no_match() {
    let config = cfg();
    let mut chars = "- rest".char_indices().peekable();
    let result = NumberRule.try_lex(&mut chars, &config);
    assert!(result.is_none());
}

#[test]
fn number_rule_no_match() {
    let config = cfg();
    let mut chars = "hello".char_indices().peekable();
    let result = NumberRule.try_lex(&mut chars, &config);
    assert!(result.is_none());
}

#[test]
fn number_rule_name() {
    let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
    assert_eq!(rule.name(), "number");
}

#[test]
fn number_rule_clone_box() {
    let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
    assert_eq!(cloned.name(), "number");
}

// Bridge test: prove `NumberRule` plugs into the object-safe `DynLexer` via
// `StaticRuleAdapter`. Dependency direction stays plugin → framework.
mod dyn_bridge {
    use super::*;
    use handl_plugin_ext_lexer::{DynLexError, DynLexRule, DynLexer, StaticRuleAdapter};
    use handl_plugin_lexer::EofFactory;
    use handl_plugin_lexer_common::MockFactory;

    #[test]
    fn lexes_through_static_adapter() {
        let factory = MockFactory::new();
        let rule: Box<dyn DynLexRule> = Box::new(StaticRuleAdapter::<
            NumberRule,
            MockKind,
            DefaultLexError,
        >::new(NumberRule, factory.clone()));
        let lexer = DynLexer::new()
            .with_rule(rule)
            .with_eof_kind(Box::new(factory.eof_kind()));

        let tokens = lexer.lex("42 -3").unwrap();
        assert_eq!(tokens.len(), 3); // 42, -3, EOF

        let n0 = tokens[0].kind.as_any().downcast_ref::<MockKind>();
        assert!(matches!(n0, Some(MockKind::Num(42.0))));
        assert_eq!(tokens[0].span, Span::new(0, 2));

        let n1 = tokens[1].kind.as_any().downcast_ref::<MockKind>();
        assert!(matches!(n1, Some(MockKind::Num(-3.0))));
        assert_eq!(tokens[1].span, Span::new(3, 5));

        assert!(tokens[2].kind.is_eof());
    }

    #[test]
    fn rejects_dash_without_digit() {
        let factory = MockFactory::new();
        let rule: Box<dyn DynLexRule> = Box::new(StaticRuleAdapter::<
            NumberRule,
            MockKind,
            DefaultLexError,
        >::new(NumberRule, factory.clone()));
        let lexer = DynLexer::new()
            .with_rule(rule)
            .with_eof_kind(Box::new(factory.eof_kind()));

        let err = lexer.lex("- hello").unwrap_err();
        assert!(matches!(err, DynLexError::UnexpectedChar { .. }));
    }
}
