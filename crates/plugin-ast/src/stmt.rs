use crate::Node;

/// Capability trait for statement-shaped nodes.
///
/// Implementors must declare the expression node type they contain.
pub trait Stmt: Node {
    /// The expression node type contained in statements.
    type Expr: Node;

    /// Creates an expression statement.
    fn expression(expr: Self::Expr) -> Self
    where
        Self: Sized;

    /// Creates a `let` statement.
    fn let_<S>(name: S, value: Self::Expr) -> Self
    where
        Self: Sized,
        S: Into<String>;

    /// Creates a block of statements.
    fn block(stmts: Vec<Self>) -> Self
    where
        Self: Sized;
}
