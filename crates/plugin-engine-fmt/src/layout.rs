//! Layout rules for the token-stream formatter (ADR-0095, stage 2).
//!
//! The layout stage decides how adjacent inline tokens join (spacing) and how
//! indentation follows bracketing. Decisions are driven by `Language` data —
//! [`classify_token`] reads `TokenKindDef::category`/`label` (ADR-0044) — so
//! the spacing vocabulary is language-generic, matching the AST emitter's
//! grammar-driven rendering.
//!
//! Each concrete canonical style is a [`FormattingRule`]; the v0.1 canonical
//! spacer (`needs_space` traffic, ADR-0086) is [`CanonicalFormattingRule`]. A
//! different dialect or style swaps in new rule objects without touching the
//! render stage ([`crate::token::TokenFormatter`]).

use handl_language::Language;

/// Token categories for canonical spacing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenCat {
    /// `(`, `[`
    OpenBracket,
    /// `)`
    CloseParen,
    /// `]`
    CloseBracket,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// Binary operator (space around).
    BinaryOp,
    /// Keyword (space after).
    Keyword,
    /// `--` or `---` comment.
    Comment,
    /// Identifier, number, string, bool, null.
    Atom,
    /// Anything else.
    Other,
}

/// Bracketing direction of a category, used by the render stage to track
/// indentation depth (the calculus vocabulary — cf. Pratt's prefix/infix loop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentBias {
    /// Opening bracket: indent after emitting.
    Open,
    /// Closing bracket: unindent before emitting.
    Close,
    /// Not a bracket: no indentation change.
    None,
}

/// Classify a token kind into a spacing category.
///
/// Driven by the language's `TokenKindDef::category`/`label` data
/// (ADR-0044): brackets resolve via `open-bracket`/`close-bracket` +
/// spelling, binary operators via the `operator` category minus the
/// non-infix spellings (`!`, `?`, `...`, `|` — prefix/nullary or
/// non-spacing operators in v0.1 surface).
pub fn classify_token(lang: &Language, kind: &str) -> TokenCat {
    let is_label = |l: &str| lang.token_kind_for_label(l) == Some(kind);
    match lang.token_category(kind) {
        Some("open-bracket") => {
            if is_label("{") {
                TokenCat::OpenBrace
            } else {
                TokenCat::OpenBracket
            }
        }
        Some("close-bracket") => {
            if is_label("}") {
                TokenCat::CloseBrace
            } else if is_label(")") {
                TokenCat::CloseParen
            } else {
                TokenCat::CloseBracket
            }
        }
        Some("comment") | Some("doc-comment") => TokenCat::Comment,
        // Binary (infix, space-around) operators: the `operator` category
        // minus the non-infix spellings.
        Some("operator") => {
            match lang.token_kind(kind).and_then(|d| d.label.as_deref()) {
                // Non-infix spellings (plus unlabeled legacy kinds like
                // `token.bang`/`token.question`) are not binary operators.
                Some("!") | Some("?") | Some("...") | Some("|") | None => TokenCat::Other,
                Some(_) => TokenCat::BinaryOp,
            }
        }
        Some("keyword") => TokenCat::Keyword,
        Some("identifier") | Some("number") | Some("string") | Some("bool") | Some("null")
        | Some("template") => TokenCat::Atom,
        Some("punctuation") => {
            if is_label(",") {
                TokenCat::Comma
            } else if is_label(":") {
                TokenCat::Colon
            } else if is_label(".") {
                TokenCat::Dot
            } else {
                TokenCat::Other
            }
        }
        _ => TokenCat::Other,
    }
}

/// Bracket direction of a category, for depth tracking.
pub fn indent_bias(cat: TokenCat) -> IndentBias {
    match cat {
        TokenCat::OpenBracket | TokenCat::OpenBrace => IndentBias::Open,
        TokenCat::CloseParen | TokenCat::CloseBracket | TokenCat::CloseBrace => IndentBias::Close,
        _ => IndentBias::None,
    }
}

/// A single, composable layout rule.
///
/// A rule decides how two adjacent inline tokens join (spacing). Rules are
/// pure functions of token categories, so the render stage
/// ([`crate::token::TokenFormatter`]) is agnostic to which style is active.
/// This is the formatter analog of the test runner's `TestProvider` seam
/// (ADR-0089 §"Providers (extensible from day one)"): reordering or replacing
/// a rule is composition, not surgery on the render loop.
pub trait FormattingRule {
    /// Short name for diagnostics / rule selection.
    fn name(&self) -> &str;

    /// Whether a single space separates an inline `prev`..`curr` pair.
    fn needs_space(&self, prev: TokenCat, curr: TokenCat) -> bool;
}

/// The canonical v0.1 spacing rule set (ADR-0086 §Canonical style).
///
/// Implements the v0.1 precedence/spacing table. Kept as a named rule so a
/// host or kit can swap it for another dialect's spacer without touching the
/// render stage.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalFormattingRule;

impl FormattingRule for CanonicalFormattingRule {
    fn name(&self) -> &str {
        "canonical-spacing"
    }

    fn needs_space(&self, prev: TokenCat, curr: TokenCat) -> bool {
        canonical_needs_space(prev, curr)
    }
}

/// Decide whether a single space should appear between two adjacent inline
/// tokens (the v0.1 canonical table).
fn canonical_needs_space(prev: TokenCat, curr: TokenCat) -> bool {
    use TokenCat::*;
    // No space inside () and [].
    if prev == OpenBracket {
        return false;
    }
    if curr == CloseParen || curr == CloseBracket {
        return false;
    }
    // Space before closing brace (inline record: value }), but not for
    // empty {} or when the brace opened a block (line-start handled elsewhere).
    if curr == CloseBrace {
        if prev == OpenBrace {
            return false; // empty {}
        }
        return true;
    }
    // No space before comma / colon / dot.
    if curr == Comma || curr == Colon || curr == Dot {
        return false;
    }
    // No space after dot (field access: c.sort).
    if prev == Dot {
        return false;
    }
    // Space after comma.
    if prev == Comma {
        return true;
    }
    // Space after colon (record entry: key: value).
    if prev == Colon {
        return true;
    }
    // Space around binary operators.
    if prev == BinaryOp || curr == BinaryOp {
        return true;
    }
    // Space after keywords.
    if prev == Keyword || curr == Keyword {
        return true;
    }
    // Space before { (if x {, match x {).
    if curr == OpenBrace {
        return true;
    }
    // Space after { (inline record: { key).
    if prev == OpenBrace {
        return true;
    }
    // No space before ( in function calls: sort(...), loop(...).
    if curr == OpenBracket && (prev == Atom || prev == CloseParen || prev == CloseBracket) {
        return false;
    }
    // Default: no space.
    false
}

/// Free-function spelling of the canonical spacing decision.
///
/// Convenience for callers that want the v0.1 rule without instantiating a
/// [`CanonicalFormattingRule`] (e.g. back-compat tests).
pub fn needs_space(prev: TokenCat, curr: TokenCat) -> bool {
    let rule = CanonicalFormattingRule;
    rule.needs_space(prev, curr)
}