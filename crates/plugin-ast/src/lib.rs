//! # handl-plugin-ast
//!
//! Generic, trait-based AST abstractions for the handl language.
//!
//! Defines the pluggable `Node` trait and the per-shape capability traits
//! `Expr`, `Stmt`, `Pattern`, `Type`, and `Field`, alongside a generic
//! `ConcreteNode` container and common metadata types (`Span`, `Attrs`,
//! `Both`) and factory traits.

mod concrete_node;
mod expr;
mod expr_factory;
mod field;
mod metadata;
mod node;
mod pattern;
mod stmt;
mod stmt_factory;
mod type_;
mod type_factory;

#[cfg(test)]
mod tests;

pub use concrete_node::Node as ConcreteNode;
pub use expr::Expr;
pub use expr_factory::ExprFactory;
pub use field::Field;
pub use metadata::{Attrs, Both, NodeMetadata, Span};
pub use node::Node;
pub use pattern::Pattern;
pub use stmt::Stmt;
pub use stmt_factory::StmtFactory;
pub use type_::Type;
pub use type_factory::TypeFactory;
