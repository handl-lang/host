use std::fmt::Debug;

/// Metadata attached to an AST node.
///
/// `()` is the zero-sized "no metadata" instance. Concrete metadata types
/// (`Span`, `Attrs`, `Both`) implement this trait so they can be used as
/// the `Metadata` associated type on the `Node` trait.
pub trait NodeMetadata: Default + Clone + Debug {}

impl NodeMetadata for () {}
