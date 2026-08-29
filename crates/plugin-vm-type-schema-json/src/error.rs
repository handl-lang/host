use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SchemaError {
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
    #[error("unknown $ref: {0}")]
    UnknownRef(String),
    #[error("recursion depth limit exceeded at {0}")]
    RecursionLimit(String),
    #[error("generic argument count mismatch for {name}: expected {expected}, found {found}")]
    ArgCountMismatch {
        name: String,
        expected: usize,
        found: usize,
    },
}
