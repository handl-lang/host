use super::rule::lex_template;
use super::segment::TemplateSegment;
use handl_plugin_lexer::Span;

#[test]
fn lex_simple_text() {
    let mut chars = "`hello`".char_indices().peekable();
    let (segments, span) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("hello".into())]);
    assert_eq!(span, Span::new(0, 7));
}

#[test]
fn lex_with_interpolation() {
    let mut chars = "`hello ${name}!`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Text("hello ".into()),
            TemplateSegment::Expr("name".into()),
            TemplateSegment::Text("!".into()),
        ]
    );
}

#[test]
fn lex_multiple_interpolations() {
    let mut chars = "`${a} and ${b}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Expr("a".into()),
            TemplateSegment::Text(" and ".into()),
            TemplateSegment::Expr("b".into()),
        ]
    );
}

#[test]
fn lex_unterminated_errors() {
    let mut chars = "`oops".char_indices().peekable();
    let result = lex_template(&mut chars);
    assert!(result.is_err());
}

#[test]
fn lex_escape_sequences() {
    let mut chars = "`a\\nb\\`c`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("a\nb`c".into())]);
}

#[test]
fn lex_dollar_not_interpolation() {
    let mut chars = "`price: $5`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("price: $5".into())]);
}

#[test]
fn lex_empty_template() {
    let mut chars = "``".char_indices().peekable();
    let (segments, span) = lex_template(&mut chars).unwrap();
    assert!(segments.is_empty());
    assert_eq!(span, Span::new(0, 2));
}

#[test]
fn lex_nested_braces_in_expr() {
    let mut chars = "`${obj.field}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Expr("obj.field".into())]);
}

#[test]
fn lex_complex_expr_with_braces() {
    let mut chars = "`result: ${map.get({key: val})}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Text("result: ".into()),
            TemplateSegment::Expr("map.get({key: val})".into()),
        ]
    );
}

#[test]
fn lex_multiline_template() {
    let mut chars = "`line1\nline2\n${x}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Text("line1\nline2\n".into()),
            TemplateSegment::Expr("x".into()),
        ]
    );
}

#[test]
fn lex_unterminated_interpolation() {
    let mut chars = "`hello ${name`".char_indices().peekable();
    let result = lex_template(&mut chars);
    assert!(result.is_err());
}

#[test]
fn lex_escape_at_eof() {
    let mut chars = "`hello\\`".char_indices().peekable();
    let result = lex_template(&mut chars);
    assert!(result.is_err());
}

#[test]
fn lex_empty_interpolation() {
    let mut chars = "`${}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Expr("".into())]);
}

#[test]
fn lex_dollar_at_eof() {
    let mut chars = "`hello$`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("hello$".into())]);
}

#[test]
fn lex_dollar_followed_by_non_brace() {
    let mut chars = "`$5 and $10`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("$5 and $10".into())]);
}

#[test]
fn lex_escape_dollar_brace() {
    let mut chars = "`\\${not interp}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![TemplateSegment::Text("${not interp}".into())]
    );
}

#[test]
fn lex_escape_backtick_in_expr() {
    // Inside ${...}, escapes are not processed — raw text is captured as-is
    let mut chars = "`${a\\`b}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Expr("a\\`b".into())]);
}

#[test]
fn lex_only_interpolation() {
    let mut chars = "`${expr}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Expr("expr".into())]);
}

#[test]
fn lex_text_then_interpolation_no_trailing() {
    let mut chars = "`text${x}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Text("text".into()),
            TemplateSegment::Expr("x".into()),
        ]
    );
}

#[test]
fn lex_interpolation_then_text_no_leading() {
    let mut chars = "`${x}text`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Expr("x".into()),
            TemplateSegment::Text("text".into()),
        ]
    );
}

#[test]
fn lex_all_escape_sequences() {
    let mut chars = "`\\n\\t\\r\\\\\\`\\$\\{\\}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![TemplateSegment::Text("\n\t\r\\`${}".into())]
    );
}

#[test]
fn lex_unknown_escape_passes_through() {
    let mut chars = "`\\x`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Text("x".into())]);
}

#[test]
fn lex_deeply_nested_braces() {
    let mut chars = "`${{{a}}}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(segments, vec![TemplateSegment::Expr("{{a}}".into())]);
}

#[test]
fn lex_consecutive_interpolations() {
    let mut chars = "`${a}${b}`".char_indices().peekable();
    let (segments, _) = lex_template(&mut chars).unwrap();
    assert_eq!(
        segments,
        vec![
            TemplateSegment::Expr("a".into()),
            TemplateSegment::Expr("b".into()),
        ]
    );
}
