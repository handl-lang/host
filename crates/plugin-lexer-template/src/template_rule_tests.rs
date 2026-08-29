use super::rule::TemplateRule;
use super::segment::TemplateSegment;
use super::token_kind::TemplateTokenKind;
use handl_plugin_lexer::{
    DefaultLexError, EofFactory, LexError, LexerConfig, LexRule, Span, Token, TokenKind,
};

// --- Mock token kind for testing the LexRule impl ---

#[derive(Debug, Clone, PartialEq)]
enum MockKind {
    Eof,
    Template(Vec<TemplateSegment>),
}

impl TokenKind for MockKind {
    type Factory = MockFactory;
    fn is_eof(&self) -> bool {
        matches!(self, MockKind::Eof)
    }
    fn is_error(&self) -> bool {
        false
    }
}

impl std::fmt::Display for MockKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MockKind::Eof => write!(f, "eof"),
            MockKind::Template(segs) => {
                write!(f, "template(")?;
                for s in segs {
                    match s {
                        TemplateSegment::Text(t) => write!(f, "{t:?}")?,
                        TemplateSegment::Expr(e) => write!(f, "${{{e}}}")?,
                    }
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct MockFactory;

impl EofFactory<MockKind> for MockFactory {
    fn eof_kind(&self) -> MockKind {
        MockKind::Eof
    }
}

impl TemplateTokenKind for MockKind {
    fn template_kind(segments: Vec<TemplateSegment>) -> Self {
        MockKind::Template(segments)
    }
}

// --- Tests ---

#[test]
fn try_lex_matches_backtick() {
    let rule = TemplateRule;
    let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory);
    let mut chars = "`hello`".char_indices().peekable();
    let result = rule.try_lex(&mut chars, &config);
    assert!(result.is_some());
    let token = result.unwrap().unwrap();
    assert_eq!(
        token.kind,
        MockKind::Template(vec![TemplateSegment::Text("hello".into())])
    );
}

#[test]
fn try_lex_does_not_match_non_backtick() {
    let rule = TemplateRule;
    let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory);
    let mut chars = "hello".char_indices().peekable();
    let result = rule.try_lex(&mut chars, &config);
    assert!(result.is_none());
}

#[test]
fn try_lex_returns_error_for_unterminated() {
    let rule = TemplateRule;
    let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory);
    let mut chars = "`oops".char_indices().peekable();
    let result = rule.try_lex(&mut chars, &config);
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

#[test]
fn rule_name_is_template() {
    let rule = TemplateRule;
    assert_eq!(
        <TemplateRule as LexRule<MockKind, DefaultLexError>>::name(&rule),
        "template"
    );
}

#[test]
fn rule_clone_box() {
    let rule = TemplateRule;
    let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = rule.clone_box();
    assert_eq!(
        <dyn LexRule<MockKind, DefaultLexError>>::name(cloned.as_ref()),
        "template"
    );
}

#[test]
fn try_lex_with_interpolation() {
    let rule = TemplateRule;
    let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory);
    let mut chars = "`hi ${name}!`".char_indices().peekable();
    let token = rule.try_lex(&mut chars, &config).unwrap().unwrap();
    assert_eq!(
        token.kind,
        MockKind::Template(vec![
            TemplateSegment::Text("hi ".into()),
            TemplateSegment::Expr("name".into()),
            TemplateSegment::Text("!".into()),
        ])
    );
}

#[test]
fn try_lex_empty_template() {
    let rule = TemplateRule;
    let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory);
    let mut chars = "``".char_indices().peekable();
    let token = rule.try_lex(&mut chars, &config).unwrap().unwrap();
    assert_eq!(token.kind, MockKind::Template(vec![]));
    assert_eq!(token.span, Span::new(0, 2));
}
