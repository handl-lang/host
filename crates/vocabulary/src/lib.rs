//! # handl-vocabulary
//!
//! Shared traits for in-process vocabulary plugins: document key
//! vocabularies and schema vocabularies.

pub mod document;
pub mod schema;

pub use document::{DocumentVocabularyPlugin, KeyKind};
pub use schema::SchemaVocabularyPlugin;
