use crate::expr_parser::ExprParser;

/// A dialect for parsing and evaluating handl expressions and documents.
///
/// `Dialect` is a facade that groups the remaining pluggable sub-traits:
///
/// - [`ExprParser`] — call detection, argument splitting, literal parsing.
///
/// Syntax tokens and document vocabulary key names have moved to `handl-syntax`.
/// Schema vocabulary has moved to `handl-schema`.
pub trait Dialect: Send + Sync {
    /// The expression parser — call detection, argument splitting, literal parsing.
    fn parser(&self) -> &dyn ExprParser;
}
