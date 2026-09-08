use handl_platform::EvalError;
use std::fmt;

/// Errors produced by HTTP server implementations.
#[derive(Debug, Clone)]
pub enum HttpServerError {
    Network { message: String },
    InvalidArgument { message: String },
    NotRunning,
    AlreadyRunning,
    Handler { message: String },
    NotFound,
    PayloadTooLarge { limit: usize },
    Timeout { message: String },
}

impl fmt::Display for HttpServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { message } => write!(f, "network error: {message}"),
            Self::InvalidArgument { message } => write!(f, "invalid argument: {message}"),
            Self::NotRunning => write!(f, "server is not running"),
            Self::AlreadyRunning => write!(f, "server is already running"),
            Self::Handler { message } => write!(f, "handler error: {message}"),
            Self::NotFound => write!(f, "not found"),
            Self::PayloadTooLarge { limit } => {
                write!(f, "payload too large: limit is {limit} bytes")
            }
            Self::Timeout { message } => write!(f, "timeout: {message}"),
        }
    }
}

impl std::error::Error for HttpServerError {}

impl From<HttpServerError> for EvalError {
    fn from(err: HttpServerError) -> Self {
        EvalError::Other(err.to_string())
    }
}

impl HttpServerError {
    pub fn into_vm_error(self) -> EvalError {
        self.into()
    }
}
