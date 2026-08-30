//! [`KeywordKind`] — open keyword token kind carrying the keyword name.
//!
//! Unlike a closed keyword enum, [`KeywordKind`] represents any keyword as a
//! `&'static str`. New keywords are added by constructing `KeywordKind("name")`
//! — no central enum to edit. Specific keywords that warrant their own type
//! (e.g. `AsKind`, `HandleKind`) can coexist as separate `DynTokenKind` impls.

use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::DynTokenKind;
use std::any::Any;
use std::fmt;

/// Token kind for keyword tokens, carrying the keyword name.
#[derive(Debug, Clone, PartialEq)]
pub struct KeywordKind(pub &'static str);

impl KeywordKind {
    /// Returns the keyword name.
    pub fn name(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DynTokenKind for KeywordKind {
    fn is_eof(&self) -> bool {
        false
    }

    fn is_error(&self) -> bool {
        false
    }

    fn eq_kind(&self, other: &dyn DynTokenKind) -> bool {
        other
            .as_any()
            .downcast_ref::<KeywordKind>()
            .map(|k| k.0 == self.0)
            .unwrap_or(false)
    }

    fn dyn_clone(&self) -> Box<dyn DynTokenKind> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// All keyword names recognized by the canonical dialect, in priority order.
pub const CANONICAL_KEYWORDS: &[&str] = &[
    "match",
    "handle",
    "as",
    "typeof",
    "not",
    "import",
    "export",
    "module",
];

/// Grammar contributor for keyword highlighting.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        let alt = CANONICAL_KEYWORDS
            .iter()
            .map(|s| regex_escape(s))
            .collect::<Vec<_>>()
            .join("|");
        vec![GrammarRule::match_rule(
            "keywords",
            "keyword",
            &format!(r"\b({alt})\b"),
            50,
        )]
    }
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$'
            | '/' => format!("\\{c}"),
            _ => c.to_string(),
        })
        .collect()
}
