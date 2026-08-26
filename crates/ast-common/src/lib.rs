//! # handl-ast-common
//!
//! Common, reusable AST toolkit built on `handl-ast`.
//!
//! Re-exports the generic abstractions from `handl-ast` and adds the most
//! commonly needed metadata types: `Span`, `Attrs`, and `Both`.

pub use handl_ast::*;

pub mod expr;
pub mod metadata;
pub mod stmt;
pub mod type_;

pub use expr::ExprFactory;
pub use metadata::{Attrs, Both, Span};
pub use stmt::StmtFactory;
pub use type_::TypeFactory;

#[cfg(test)]
mod tests;
