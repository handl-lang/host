//! Shared test stub for the dialect facade.

use handl_vm_values_json::NonFunctionValue;

use crate::dialect::Dialect;
use crate::expr_parser::ExprParser;
use crate::schema_vocabulary::SchemaVocabulary;

use crate::schema_vocabulary::schema_vocabulary_stub::StubVocabulary;

/// A minimal dialect implementation for testing.
pub struct StubDialect;

impl Dialect for StubDialect {
    fn parser(&self) -> &dyn ExprParser {
        self
    }

    fn schema_vocabulary(&self) -> &dyn SchemaVocabulary<NonFunctionValue> {
        &StubVocabulary
    }
}

/// A static [`StubDialect`] instance for convenience.
pub static STUB_DIALECT: StubDialect = StubDialect;
