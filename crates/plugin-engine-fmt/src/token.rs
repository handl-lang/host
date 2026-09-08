//! Token-stream render stage (ADR-0095, stage 3).
//!
//! [`TokenFormatter`] turns a lexed token stream into laid-out source text by
//! walking tokens in order, preserving the original line structure (newlines
//! between tokens) while applying the active [`FormattingRule`]'s spacing
//! decisions and bracket-tracking indentation. It is the "render" half of the
//! canonical source→source pipeline; the post-processing stage
//! ([`crate::post_process`]) follows it.
//!
//! The formatter is agnostic to which style is active: spacing comes from the
//! injected [`FormattingRule`], indentation from [`crate::layout::indent_bias`]
//! (the calculus vocabulary), and indentation width from the configured
//! `indent_size`.

use handl_language::{Language, Token};

use crate::layout::{
    CanonicalFormattingRule, FormattingRule, IndentBias, TokenCat, classify_token, indent_bias,
};

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
    /// between tokens) while applying the rule's spacing decisions and
    /// 2-space indentation for continuation lines.
    pub fn format(&self, src: &str, tokens: &[Token], lang: &Language) -> String {
        let starts = compute_token_starts(src, tokens);
        let mut out = String::new();
        let mut depth: usize = 0;
        let mut at_line_start = true;
        let mut prev_cat: Option<TokenCat> = None;

        for (i, token) in tokens.iter().enumerate() {
            let cat = classify_token(lang, &token.kind);
            let token_start = starts[i];
            let prev_end = if i > 0 {
                starts[i - 1] + tokens[i - 1].text.len()
            } else {
                0
            };
            let has_newline = gap_has_newline(src, prev_end, token_start);

            // Comments carry their own trailing newline; emit them as-is with
            // indentation, and force the next token to a new line.
            if cat == TokenCat::Comment {
                if !at_line_start {
                    out.push('\n');
                }
                out.push_str(&indent_str(depth * self.indent_size));
                out.push_str(&token.text);
                at_line_start = token.text.ends_with('\n');
                prev_cat = Some(cat);
                continue;
            }

            // Closing brackets decrease depth before emitting.
            if indent_bias(cat) == IndentBias::Close {
                depth = depth.saturating_sub(1);
            }

            if at_line_start {
                out.push_str(&indent_str(depth * self.indent_size));
                at_line_start = false;
            } else if has_newline {
                out.push('\n');
                out.push_str(&indent_str(depth * self.indent_size));
            } else if let Some(prev) = prev_cat {
                if self.rule.needs_space(prev, cat) {
                    out.push(' ');
                }
            }

            out.push_str(&token.text);

            // Opening brackets increase depth after emitting.
            if indent_bias(cat) == IndentBias::Open {
                depth += 1;
            }

            prev_cat = Some(cat);
        }

        out
    }
}

/// Compute the byte start position of each token in the source by scanning
/// forward, skipping whitespace.
fn compute_token_starts(src: &str, tokens: &[Token]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(tokens.len());
    let bytes = src.as_bytes();
    let mut pos = 0;
    for token in tokens {
        // Skip whitespace.
        while pos < bytes.len() {
            let ch = match src[pos..].chars().next() {
                Some(c) => c,
                None => break,
            };
            if ch.is_whitespace() {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }
        starts.push(pos);
        // Advance past the token text.
        pos += token.text.len();
    }
    starts
}

/// Check whether the source text between two byte offsets contains a newline.
fn gap_has_newline(src: &str, from: usize, to: usize) -> bool {
    let from = from.min(src.len());
    let to = to.min(src.len());
    src[from..to].contains('\n')
}

/// Produce the indentation string for the given total width (in spaces).
fn indent_str(cols: usize) -> String {
    " ".repeat(cols)
}