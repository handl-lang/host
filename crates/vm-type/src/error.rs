use thiserror::Error;

use crate::core::{Kind, Level, Name, Var};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TypeError {
    #[error("cannot synthesize type for {0}")]
    CannotSynthesize(String),
    #[error("type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    #[error("kind mismatch: expected {expected}, found {found}")]
    KindMismatch { expected: String, found: String },
    #[error("expected function kind, found {0}")]
    ExpectedFunctionKind(String),
    #[error("expected function type, found {0}")]
    ExpectedFunction(String),
    #[error("expected sigma type, found {0}")]
    ExpectedSigma(String),
    #[error("undefined variable {0:?}")]
    UndefinedVariable(Var),
    #[error("undefined type variable {0}")]
    UndefinedTypeVariable(Name),
    #[error("undefined alias {0}")]
    AliasNotFound(Name),
    #[error("alias depth limit exceeded for {0}")]
    AliasDepthLimit(Name),
    #[error("alias cycle detected at {0}")]
    AliasCycle(Name),
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
    #[error("occurs check failed for type variable {0}")]
    TVarOccursCheck(String),
    #[error("mismatch between {0} and {1}")]
    Mismatch(String, String),
    #[error("cannot unify levels {0:?} and {1:?}")]
    LevelMismatch(Level, Level),
    #[error("kind mismatch: expected {expected}, found {found}")]
    KindMismatch { expected: Kind, found: Kind },
}
