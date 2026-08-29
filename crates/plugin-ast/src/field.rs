use crate::Node;

/// Capability trait for field-shaped nodes.
///
/// A field is a named value binding.
pub trait Field: Node {
    /// Creates a new field.
    fn new<S>(name: S, value: Self::Value) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Returns the field name.
    fn name(&self) -> &str;

    /// Returns the field value.
    fn value(&self) -> &Self::Value;
}
