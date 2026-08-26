use std::fmt;

use handl_vm_values_json::Error as VmError;

/// Errors that can occur in TLS operations.
#[derive(Debug, Clone)]
pub enum TlsError {
    /// The input argument was not a string or an array of strings.
    InvalidInput { message: String },
    /// Certificate generation failed.
    Generation { message: String },
    /// The PEM input could not be parsed.
    InvalidPem { message: String },
    /// A fingerprint could not be computed.
    Fingerprint { message: String },
    /// The configuration could not be built.
    InvalidConfig { message: String },
    /// An I/O or transport error occurred.
    Io { message: String },
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { message } => write!(f, "invalid input: {message}"),
            Self::Generation { message } => write!(f, "certificate generation failed: {message}"),
            Self::InvalidPem { message } => write!(f, "invalid PEM: {message}"),
            Self::Fingerprint { message } => write!(f, "fingerprint failed: {message}"),
            Self::InvalidConfig { message } => write!(f, "invalid config: {message}"),
            Self::Io { message } => write!(f, "tls I/O error: {message}"),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<TlsError> for VmError {
    fn from(err: TlsError) -> Self {
        VmError::PrimitiveError(err.to_string())
    }
}

impl TlsError {
    /// Converts this error into a VM primitive error.
    pub fn into_vm_error(self) -> VmError {
        self.into()
    }
}
