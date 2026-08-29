//! # handl-schema
//!
//! Schema validation vocabulary for the handl language.
//!
//! This crate provides the generic [`Schema`] and [`SchemaVocabulary`]
//! abstractions, the [`ValidationError`] type, and the canonical
//! JSON Schema draft 2020-12 implementation, [`JsonSchemaVocabulary`].

#![allow(clippy::module_inception)]

pub mod json_schema_vocabulary;
pub mod schema;
pub mod schema_vocabulary;
pub mod validation_error;

pub use json_schema_vocabulary::JsonSchemaVocabulary;
pub use schema::Schema;
pub use schema_vocabulary::SchemaVocabulary;
pub use validation_error::ValidationError;

#[cfg(test)]
mod validation_error_tests;

#[cfg(test)]
mod json_schema_vocabulary_tests;
