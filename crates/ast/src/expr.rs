use crate::Node;

/// Capability trait for expression-shaped nodes.
///
/// A plugin that provides expressions implements this on its concrete
/// node type.
pub trait Expr: Node {
    /// Creates a literal value expression.
    fn value(value: Self::Value) -> Self
    where
        Self: Sized;

    /// Creates a host-provided function call.
    fn call<S>(name: S, args: Vec<Self>) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Creates an invocation of an arbitrary function value.
    fn apply(target: Self, args: Vec<Self>) -> Self
    where
        Self: Sized;

    /// Creates an array literal expression.
    fn array(elements: Vec<Self>) -> Self
    where
        Self: Sized;

    /// Creates an object literal expression.
    fn object(entries: Vec<(String, Self)>) -> Self
    where
        Self: Sized;

    /// Creates a postfix index expression.
    fn index(target: Self, index: Self) -> Self
    where
        Self: Sized;
}
