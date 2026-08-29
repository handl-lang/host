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

#[cfg(test)]
mod rule_tests;
