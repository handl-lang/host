//! Shared test stub for the dialect facade.

use crate::dialect::Dialect;
use crate::expr_parser::ExprParser;

/// A minimal dialect implementation for testing.
pub struct StubDialect;

impl Dialect for StubDialect {
    fn parser(&self) -> &dyn ExprParser {
        self
    }
}

/// A static [`StubDialect`] instance for convenience.
pub static STUB_DIALECT: StubDialect = StubDialect;
