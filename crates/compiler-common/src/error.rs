use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum CompilerError {
    #[error("pass failed: {0}")]
    PassFailed(String),
    #[error("unknown pass: {0}")]
    UnknownPass(String),
    #[error("pipeline failed")]
    PipelineFailed,
}
