//! Grammar rule for template string literals.
//!
//! [`TemplateRule`] is a prefix grammar rule that handles template
//! tokens produced by `plugin-lexer-template`. It produces an AST
//! node of kind `"template"` with a `segments` array containing
//! `{"type": "text", "value": "..."}` and
//! `{"type": "expr", "raw": "..."}` entries.

pub mod rule;
pub mod token;

pub use rule::TemplateRule;
pub use token::TemplateToken;

use handl_plugin_lexer_template::TemplateSegment;
use handl_plugin_token_kind::LexKind;

impl TemplateToken for LexKind {
    fn as_template_segments(&self) -> Option<&[TemplateSegment]> {
        match self {
            LexKind::Template(segments) => Some(segments.as_slice()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod rule_tests;
