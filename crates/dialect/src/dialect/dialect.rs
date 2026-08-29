/// A dialect for parsing and evaluating handl expressions and documents.
///
/// `Dialect` is a facade for remaining pluggable sub-traits.
///
/// Syntax tokens and document vocabulary key names have moved to `handl-syntax`.
/// Schema vocabulary has moved to `handl-schema`.
pub trait Dialect: Send + Sync {}
