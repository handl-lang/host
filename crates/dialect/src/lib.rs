//! # handl-dialect
//!
//! Dialect abstraction for the handl language implementation.
//!
//! This crate currently provides the [`Dialect`] facade and the
//! [`ExprParser`] abstraction. Syntax tokens and document vocabulary key
//! names have moved to `handl-syntax` as `const` configuration structs
//! (`CANONICAL_SYNTAX` and `CANONICAL_DOCUMENT`). Schema vocabulary has
//! moved to `handl-schema`.

#![allow(clippy::module_inception)]

pub mod dialect;
mod expr_parser;

pub use dialect::Dialect;
pub use expr_parser::{ExprParser, ParsedHandlerClause};
