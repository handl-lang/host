/// Capability trait for pattern-shaped values.
///
/// Patterns appear in binding positions. They are not required to carry
/// the full `Node` metadata contract, but may do so when a plugin
/// chooses to represent them as first-class nodes.
pub trait Pattern {
    /// Creates an identifier binding pattern.
    fn identifier<S>(name: S) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Creates an array destructuring pattern.
    fn array(elements: Vec<Self>) -> Self
    where
        Self: Sized;

    /// Creates an object destructuring pattern.
    fn object(entries: Vec<(String, Self)>) -> Self
    where
        Self: Sized;

    /// Creates a rest pattern.
    fn rest<S>(name: S) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Returns the variable names bound by this pattern.
    fn bound_names(&self) -> Vec<String>;
}
