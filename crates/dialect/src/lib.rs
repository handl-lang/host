//! # handl-dialect
//!
//! Dialect abstraction for the handl language implementation.
//!
//! This crate currently provides the [`Dialect`] facade along with the
//! [`ExprParser`] and [`SchemaVocabulary`] abstractions. Syntax tokens and
//! document vocabulary key names have moved to `handl-syntax` as `const`
//! configuration structs (`CANONICAL_SYNTAX` and `CANONICAL_DOCUMENT`).

#![allow(clippy::module_inception)]

pub mod dialect;
mod expr_parser;
mod schema_vocabulary;

pub use dialect::Dialect;
pub use expr_parser::{ExprParser, ParsedHandlerClause};
pub use schema_vocabulary::{Schema, SchemaVocabulary, ValidationError};
