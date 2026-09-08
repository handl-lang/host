use handl_platform::EvalError as VmError;
use std::fmt;

/// Errors produced by HTTP service implementations.
///
/// Converts to [`EvalError`] via `From`, so service methods can return
/// `Result<T, HttpError>` and the adapter handles the conversion.
#[derive(Debug, Clone)]
pub enum HttpError {
    Network { message: String },
    InvalidArgument { message: String },
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { message } => write!(f, "network error: {message}"),
            Self::InvalidArgument { message } => write!(f, "invalid argument: {message}"),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<HttpError> for VmError {
    fn from(err: HttpError) -> Self {
        VmError::Other(err.to_string())
    }
}

impl HttpError {
    /// Converts into a [`VmError`].
    pub fn into_vm_error(self) -> VmError {
        self.into()
    }
}
