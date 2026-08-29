use handl_vm_values_json::NonFunctionValue;

use crate::expr_parser::ExprParser;
use crate::schema_vocabulary::SchemaVocabulary;

/// A dialect for parsing and evaluating handl expressions and documents.
///
/// `Dialect` is a facade that groups the remaining pluggable sub-traits:
///
/// - [`ExprParser`] — call detection, argument splitting, literal parsing.
/// - [`SchemaVocabulary`] — schema language for field and function validation.
///
/// Syntax tokens and document vocabulary key names have moved to `handl-syntax`.
pub trait Dialect: Send + Sync {
    /// The expression parser — call detection, argument splitting, literal parsing.
    fn parser(&self) -> &dyn ExprParser;

    /// The schema vocabulary used for field and function validation.
    ///
    /// The canonical dialect returns a JSON Schema draft 2020-12 vocabulary.
    fn schema_vocabulary(&self) -> &dyn SchemaVocabulary<NonFunctionValue>;
}
