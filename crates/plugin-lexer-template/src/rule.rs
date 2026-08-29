use handl_plugin_lexer::{DefaultLexError, LexError, LexerConfig, LexRule, Span, Token};

use crate::segment::TemplateSegment;
use crate::token_kind::TemplateTokenKind;

/// Matches backtick template string literals.
///
/// The rule scans from the opening backtick to the closing backtick,
/// splitting the content into literal text segments and interpolation
/// expressions. The raw text of each `${...}` is captured as an
/// `Expr(String)` segment for later re-parsing by the grammar/kit.
#[derive(Clone, Default)]
pub struct TemplateRule;

impl<K, E> LexRule<K, E> for TemplateRule
where
    K: TemplateTokenKind,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        _config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, '`')) => Some(
                lex_template(chars)
                    .map(|(segments, span)| Token::new(K::template_kind(segments), span))
                    .map_err(E::from),
            ),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "template"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

/// Lexes a backtick template string.
///
/// The opening backtick has already been peeked. This function consumes
/// the entire template literal and returns its segments and span.
pub(crate) fn lex_template(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<(Vec<TemplateSegment>, Span), DefaultLexError> {
    let (start, _) = chars.next().unwrap(); // consume opening backtick
    let mut end = start + 1;
    let mut segments = Vec::new();
    let mut current_text = String::new();

    loop {
        match chars.next() {
            None => {
                return Err(DefaultLexError::UnterminatedString {
                    span: Span::new(start, end),
                });
            }
            Some((pos, '`')) => {
                end = pos + 1;
                break;
            }
            Some((_, '$')) => {
                if let Some(&(_, '{')) = chars.peek() {
                    if !current_text.is_empty() {
                        segments.push(TemplateSegment::Text(std::mem::take(&mut current_text)));
                    }
                    chars.next(); // consume '{'
                    let mut depth = 1usize;
                    let mut expr_text = String::new();
                    loop {
                        match chars.next() {
                            None => {
                                return Err(DefaultLexError::UnterminatedString {
                                    span: Span::new(start, end),
                                });
                            }
                            Some((_, '{')) => {
                                depth += 1;
                                expr_text.push('{');
                            }
                            Some((_, '}')) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                expr_text.push('}');
                            }
                            Some((_, ch)) => {
                                expr_text.push(ch);
                            }
                        }
                    }
                    segments.push(TemplateSegment::Expr(expr_text));
                } else {
                    current_text.push('$');
                }
            }
            Some((_, '\\')) => {
                match chars.next() {
                    Some((_, 'n')) => current_text.push('\n'),
                    Some((_, 't')) => current_text.push('\t'),
                    Some((_, 'r')) => current_text.push('\r'),
                    Some((_, '\\')) => current_text.push('\\'),
                    Some((_, '`')) => current_text.push('`'),
                    Some((_, '$')) => current_text.push('$'),
                    Some((_, '{')) => current_text.push('{'),
                    Some((_, '}')) => current_text.push('}'),
                    Some((_, ch)) => current_text.push(ch),
                    None => {
                        return Err(DefaultLexError::UnterminatedString {
                            span: Span::new(start, end),
                        });
                    }
                }
            }
            Some((_, ch)) => {
                current_text.push(ch);
            }
        }
    }

    if !current_text.is_empty() {
        segments.push(TemplateSegment::Text(current_text));
    }

    Ok((segments, Span::new(start, end)))
}
