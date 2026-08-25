use crate::capability::CapabilityId;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum PlatformError {
    #[error("unknown capability: {0}")]
    UnknownCapability(CapabilityId),
    #[error("plugin error: {0}")]
    Plugin(String),
    #[error("not implemented")]
    NotImplemented,
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_capability_display() {
        let e = PlatformError::UnknownCapability("lint".into());
        assert_eq!(format!("{e}"), "unknown capability: lint");
    }

    #[test]
    fn plugin_error_display() {
        let e = PlatformError::Plugin("process crashed".into());
        assert_eq!(format!("{e}"), "plugin error: process crashed");
    }

    #[test]
    fn not_implemented_display() {
        let e = PlatformError::NotImplemented;
        assert_eq!(format!("{e}"), "not implemented");
    }

    #[test]
    fn invalid_metadata_display() {
        let e = PlatformError::InvalidMetadata("missing command".into());
        assert_eq!(format!("{e}"), "invalid metadata: missing command");
    }
}
