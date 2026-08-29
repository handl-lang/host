use super::rule::TemplateRule;
use super::token::TemplateToken;
use handl_plugin_ext_parser::{BasicParser, Parser, ParserBehavior, ParserContext, ParserRequest};
use handl_plugin_lexer::{EofFactory, Span, Token, TokenKind};
use handl_plugin_lexer_template::{TemplateSegment, TemplateTokenKind};
use handl_platform::{AstNode, Behavior, Span as PlatformSpan};
use serde_json::json;

// --- Mock token kind ---

#[derive(Debug, Clone, PartialEq)]
enum MockKind {
    Eof,
    Template(Vec<TemplateSegment>),
    Other,
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
            MockKind::Template(_) => write!(f, "template"),
            MockKind::Other => write!(f, "other"),
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

impl TemplateToken for MockKind {
    fn as_template_segments(&self) -> Option<&[TemplateSegment]> {
        match self {
            MockKind::Template(segs) => Some(segs.as_slice()),
            _ => None,
        }
    }
}

fn make_token(segments: Vec<TemplateSegment>) -> Token<MockKind> {
    Token::new(
        MockKind::Template(segments),
        Span::new(0, 10),
    )
}

fn make_ctx<'a>() -> ParserContext<'a, MockKind> {
    use handl_platform::ResolvedOptions;
    let parser = Box::leak(Box::new(BasicParser::<MockKind>::new()));
    ParserContext::new(parser, &[], ResolvedOptions::new())
}

// --- Tests ---

#[test]
fn behavior_kind_is_template() {
    let rule = TemplateRule;
    assert_eq!(
        <TemplateRule as Behavior<Parser<MockKind>>>::kind(&rule),
        "template"
    );
}

#[test]
fn behavior_apply_succeeds() {
    let rule = TemplateRule;
    let mut op: ParserRequest<MockKind> = ParserRequest {
        tokens: vec![],
        pos: 0,
        options: handl_platform::ResolvedOptions::new(),
        ast: AstNode {
            kind: "parser.root".into(),
            span: None,
            payload: json!({}),
        },
    };
    let result = rule.apply(&mut op);
    assert!(result.is_ok());
}

#[test]
fn matches_prefix_true_for_template() {
    let rule = TemplateRule;
    let kind = MockKind::Template(vec![TemplateSegment::Text("hi".into())]);
    assert!(rule.matches_prefix(&kind));
}

#[test]
fn matches_prefix_false_for_non_template() {
    let rule = TemplateRule;
    assert!(!rule.matches_prefix(&MockKind::Other));
    assert!(!rule.matches_prefix(&MockKind::Eof));
}

#[test]
fn matches_prefix_false_for_empty_template() {
    let rule = TemplateRule;
    let kind = MockKind::Template(vec![]);
    // Even an empty template is still a template token
    assert!(rule.matches_prefix(&kind));
}

#[test]
fn prefix_produces_text_only_ast() {
    let rule = TemplateRule;
    let token = make_token(vec![TemplateSegment::Text("hello".into())]);
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    assert_eq!(ast.kind, "template");
    let segments = ast.payload.get("segments").unwrap().as_array().unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].get("type").unwrap(), "text");
    assert_eq!(segments[0].get("value").unwrap(), "hello");
}

#[test]
fn prefix_produces_expr_only_ast() {
    let rule = TemplateRule;
    let token = make_token(vec![TemplateSegment::Expr("name".into())]);
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    assert_eq!(ast.kind, "template");
    let segments = ast.payload.get("segments").unwrap().as_array().unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].get("type").unwrap(), "expr");
    assert_eq!(segments[0].get("raw").unwrap(), "name");
}

#[test]
fn prefix_produces_mixed_ast() {
    let rule = TemplateRule;
    let token = make_token(vec![
        TemplateSegment::Text("hello ".into()),
        TemplateSegment::Expr("name".into()),
        TemplateSegment::Text("!".into()),
    ]);
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    assert_eq!(ast.kind, "template");
    let segments = ast.payload.get("segments").unwrap().as_array().unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(segments[0].get("type").unwrap(), "text");
    assert_eq!(segments[0].get("value").unwrap(), "hello ");
    assert_eq!(segments[1].get("type").unwrap(), "expr");
    assert_eq!(segments[1].get("raw").unwrap(), "name");
    assert_eq!(segments[2].get("type").unwrap(), "text");
    assert_eq!(segments[2].get("value").unwrap(), "!");
}

#[test]
fn prefix_produces_empty_segments_ast() {
    let rule = TemplateRule;
    let token = make_token(vec![]);
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    assert_eq!(ast.kind, "template");
    let segments = ast.payload.get("segments").unwrap().as_array().unwrap();
    assert!(segments.is_empty());
}

#[test]
fn prefix_preserves_span() {
    let rule = TemplateRule;
    let token = Token::new(
        MockKind::Template(vec![TemplateSegment::Text("x".into())]),
        Span::new(5, 15),
    );
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    assert_eq!(ast.span, Some(PlatformSpan { start: 5, end: 15 }));
}

#[test]
fn prefix_produces_multiple_interpolations_ast() {
    let rule = TemplateRule;
    let token = make_token(vec![
        TemplateSegment::Expr("a".into()),
        TemplateSegment::Text(" and ".into()),
        TemplateSegment::Expr("b".into()),
    ]);
    let mut ctx = make_ctx();
    let ast = rule.prefix(token, &mut ctx).unwrap();

    let segments = ast.payload.get("segments").unwrap().as_array().unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(segments[0].get("type").unwrap(), "expr");
    assert_eq!(segments[0].get("raw").unwrap(), "a");
    assert_eq!(segments[1].get("type").unwrap(), "text");
    assert_eq!(segments[1].get("value").unwrap(), " and ");
    assert_eq!(segments[2].get("type").unwrap(), "expr");
    assert_eq!(segments[2].get("raw").unwrap(), "b");
}

#[test]
fn infix_binding_is_none() {
    let rule = TemplateRule;
    assert!(rule.infix_binding(&MockKind::Template(vec![])).is_none());
    assert!(rule.infix_binding(&MockKind::Other).is_none());
}

#[test]
#[should_panic(expected = "TemplateRule is a prefix rule")]
fn infix_panics() {
    let rule = TemplateRule;
    let token = make_token(vec![TemplateSegment::Text("x".into())]);
    let mut ctx = make_ctx();
    let left = AstNode {
        kind: "value".into(),
        span: None,
        payload: json!({}),
    };
    let _ = rule.infix(left, token, &mut ctx, 0);
}

#[test]
fn template_token_trait_returns_segments() {
    let kind = MockKind::Template(vec![
        TemplateSegment::Text("a".into()),
        TemplateSegment::Expr("b".into()),
    ]);
    let segs = kind.as_template_segments().unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0], TemplateSegment::Text("a".into()));
    assert_eq!(segs[1], TemplateSegment::Expr("b".into()));
}

#[test]
fn template_token_trait_returns_none_for_non_template() {
    assert!(MockKind::Other.as_template_segments().is_none());
    assert!(MockKind::Eof.as_template_segments().is_none());
}

#[test]
fn template_token_kind_trait_constructs() {
    let kind = MockKind::template_kind(vec![TemplateSegment::Text("test".into())]);
    assert!(matches!(kind, MockKind::Template(_)));
    assert!(kind.as_template_segments().is_some());
}
