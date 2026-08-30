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
use handl_plugin_token_kind::{DynKind, TemplateKind};

impl TemplateToken for DynKind {
    fn as_template_segments(&self) -> Option<&[TemplateSegment]> {
        if let Some(TemplateKind(segments)) = self.downcast_ref::<TemplateKind>() {
            Some(segments.as_slice())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod rule_tests;
