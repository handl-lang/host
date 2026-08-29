//! [`TemplateRule`] — matches backtick template string literals.
//!
//! Template strings use backtick delimiters and support `${expr}`
//! interpolation, similar to JavaScript template literals.
//!
//! ## Example
//!
//! ```text
//! `Hello, ${name}! You are ${age} years old.`
//! ```
//!
//! The lexer produces a single token whose kind carries a list of
//! [`TemplateSegment`]s. Literal text becomes `Text(String)` and
//! interpolation expressions become `Expr(String)` containing the
//! raw expression text. The grammar rule + kit builder are
//! responsible for turning segments into an AST that combines
//! string concatenation with interpolated expressions.

pub mod rule;
pub mod segment;
pub mod token_kind;

pub use rule::TemplateRule;
pub use segment::TemplateSegment;
pub use token_kind::TemplateTokenKind;

#[cfg(test)]
mod rule_tests;

#[cfg(test)]
mod template_rule_tests;
