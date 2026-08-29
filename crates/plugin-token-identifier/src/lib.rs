#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarCapture, GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical identifier tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum IdentKind {
    /// A identifier token.
    Ident(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for IdentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentKind::Ident(v) => write!(f, "identifier({:?})", v),
            IdentKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for IdentKind {
    fn is_eof(&self) -> bool {
        matches!(self, IdentKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn Ident_token(value: String, span: Span) -> Token<IdentKind> {
    Token::new(IdentKind::Ident(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<IdentKind> for TokenFactory {
    fn eof_kind(&self) -> IdentKind {
        IdentKind::Eof
    }
}

const IDENTIFIER_REGEX: &str = r"[a-zA-Z_.][a-zA-Z0-9_.]*";

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        vec![
            // Type references: type.xxx
            GrammarRule::match_with_captures(
                "type_references",
                r"\btype\.([a-zA-Z_][a-zA-Z0-9_]*)",
                vec![GrammarCapture::new(1, "entity.name.type")],
                60,
            ),
            // PascalCase type names
            GrammarRule::match_rule(
                "type_names",
                "entity.name.type",
                r"\b[A-Z][a-zA-Z0-9_]+\b",
                61,
            ),
            // Binding declarations: identifier at start of line followed by : or =
            GrammarRule::match_with_captures(
                "binding_declarations",
                &format!(r"^({IDENTIFIER_REGEX})\s*[:=]"),
                vec![GrammarCapture::new(1, "variable.other.binding")],
                65,
            ),
            // Property names: identifier followed by colon
            GrammarRule::match_with_captures(
                "property_names",
                &format!(r"({IDENTIFIER_REGEX})\s*:"),
                vec![GrammarCapture::new(1, "variable.other.property")],
                66,
            ),
            // Function calls: identifier followed by open paren
            GrammarRule::match_with_captures(
                "function_calls",
                &format!(r"({IDENTIFIER_REGEX})\s*(\()"),
                vec![
                    GrammarCapture::new(1, "entity.name.function"),
                    GrammarCapture::new(2, "punctuation"),
                ],
                70,
            ),
            // Identifiers — catch-all
            GrammarRule::match_rule("identifiers", "variable", IDENTIFIER_REGEX, 100),
        ]
    }
}
