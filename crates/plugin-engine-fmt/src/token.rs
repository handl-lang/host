//! Token-stream render stage (ADR-0095, stage 3).
//!
//! [`TokenFormatter`] turns a lexed token stream into laid-out source text by
//! walking tokens in order, preserving the original line structure (newlines,
//! blank lines, and leading whitespace) while applying the active
//! [`FormattingRule`]'s spacing decisions for inline tokens. It is the "render"
//! half of the canonical source→source pipeline; the post-processing stage
//! ([`crate::post_process`]) follows it.
//!
//! The formatter is agnostic to which style is active: spacing comes from the
//! injected [`FormattingRule`]; token `start`/`end` offsets are used to recover
//! the original leading whitespace and blank lines.

use handl_language::{Language, Token};

use crate::layout::{CanonicalFormattingRule, FormattingRule, TokenCat, classify_token};

/// A token-stream renderer parameterized by a [`FormattingRule`] and an
/// indentation width.
pub struct TokenFormatter {
    /// The active spacing rule set.
    pub rule: Box<dyn FormattingRule>,
    /// Number of spaces per indentation level.
    pub indent_size: usize,
}

impl TokenFormatter {
    /// Create a renderer with a custom rule set and indentation width.
    pub fn new(rule: Box<dyn FormattingRule>, indent_size: usize) -> Self {
        Self { rule, indent_size }
    }

    /// Create a renderer with the canonical v0.1 rule set.
    pub fn canonical(indent_size: usize) -> Self {
        Self::new(Box::new(CanonicalFormattingRule), indent_size)
    }

    /// Format the token stream into laid-out source text.
    ///
    /// Walks tokens in order, preserving the original line structure (newlines
    /// and leading whitespace for each line-start token) while applying the
    /// rule's spacing decisions for inline tokens.
    pub fn format(&self, src: &str, tokens: &[Token], lang: &Language) -> String {
        let mut out = String::new();
        let mut at_line_start = true;
        let mut prev_cat: Option<TokenCat> = None;

        for (i, token) in tokens.iter().enumerate() {
            let token_start = token.start;
            let gap = if i == 0 {
                &src[0..token_start]
            } else {
                let prev_end = tokens[i - 1].end;
                &src[prev_end..token_start]
            };
            let has_newline = gap.contains('\n');

            let mut cat = classify_token(lang, &token.kind);
            // The `-` token is both infix subtraction and prefix negation. It
            // is a prefix operator when the preceding token is not a value or
            // closing bracket (i.e. it cannot be the right operand of a binary
            // minus).
            if token.text == "-" {
                if !matches!(
                    prev_cat,
                    Some(
                        TokenCat::Atom
                            | TokenCat::CloseParen
                            | TokenCat::CloseBracket
                            | TokenCat::CloseBrace
                    )
                ) {
                    cat = TokenCat::Other;
                }
            }

            if cat == TokenCat::Comment {
                if i == 0 || has_newline || at_line_start {
                    out.push_str(gap);
                } else if !at_line_start {
                    // same-line trailing comment; ensure a single space before it
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
                out.push_str(&token.text);
                at_line_start = token.text.ends_with('\n');
                prev_cat = Some(cat);
                continue;
            }

            if i == 0 || has_newline || at_line_start {
                out.push_str(gap);
            } else if let Some(prev) = prev_cat {
                if self.rule.needs_space(prev, cat) {
                    out.push(' ');
                }
            }

            out.push_str(&token.text);

            prev_cat = Some(cat);
            at_line_start = false;
        }

        out
    }
}
