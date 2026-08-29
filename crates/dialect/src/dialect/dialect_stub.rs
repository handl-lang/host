//! Shared test stub for the dialect facade.

use crate::dialect::Dialect;

/// A minimal dialect implementation for testing.
pub struct StubDialect;

impl Dialect for StubDialect {}

/// A static [`StubDialect`] instance for convenience.
pub static STUB_DIALECT: StubDialect = StubDialect;
