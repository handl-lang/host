use thiserror::Error;

use crate::core::{Level, Var};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TypeError {
    #[error("cannot synthesize type for {0}")]
    CannotSynthesize(String),
    #[error("type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    #[error("expected function type, found {0}")]
    ExpectedFunction(String),
    #[error("expected sigma type, found {0}")]
    ExpectedSigma(String),
    #[error("undefined variable {0:?}")]
    UndefinedVariable(Var),
    #[error("universe mismatch: expected a universe, found {0}")]
    UniverseMismatch(String),
    #[error("scope error: {0}")]
    ScopeError(String),
    #[error("unification failed: {0}")]
    Unification(#[from] UnificationError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UnificationError {
    #[error("occurs check failed for {0:?}")]
    OccursCheck(u32),
    #[error("mismatch between {0} and {1}")]
    Mismatch(String, String),
    #[error("cannot unify levels {0:?} and {1:?}")]
    LevelMismatch(Level, Level),
}
