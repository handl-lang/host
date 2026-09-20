use thiserror::Error;

/// Errors that can occur when encoding or decoding.
#[derive(Debug, Error)]
pub enum EncodingError {
    #[error("encoding error: {0}")]
    Message(String),
}

impl EncodingError {
    /// Create a new encoding error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
