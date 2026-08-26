//! # handl-ast
//!
//! Generic, trait-based AST abstractions for the handl language.
//!
//! Defines the pluggable `Node` trait and the per-shape capability traits
//! `Expr`, `Stmt`, `Pattern`, `Type`, and `Field`.

mod expr;
mod field;
mod metadata;
mod node;
mod pattern;
mod stmt;
mod type_;

pub use expr::Expr;
pub use field::Field;
pub use metadata::NodeMetadata;
pub use node::Node;
pub use pattern::Pattern;
pub use stmt::Stmt;
pub use type_::Type;
