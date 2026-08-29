use crate::Node;

/// Capability trait for type-shaped nodes.
pub trait Type: Node {
    /// Creates a primitive (named) type.
    fn primitive<S>(name: S) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Creates an array type.
    fn array(element: Self) -> Self
    where
        Self: Sized;

    /// Creates an object type.
    fn object(fields: Vec<(String, Self)>) -> Self
    where
        Self: Sized;

    /// Creates a function type.
    fn function(params: Vec<Self>, ret: Self) -> Self
    where
        Self: Sized;
}
