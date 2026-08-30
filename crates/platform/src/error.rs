//! Platform error types.

use crate::capability::CapabilityId;
use crate::eval::EvalContext;
use thiserror::Error;

/// Errors that can be raised by platform operations.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum PlatformError {
    /// A requested capability is not provided by any plugin.
    #[error("unknown capability: {0}")]
    UnknownCapability(CapabilityId),
    /// A plugin reported an error.
    #[error("plugin error: {0}")]
    Plugin(String),
    /// The requested operation is not yet implemented.
    #[error("not implemented")]
    NotImplemented,
    /// Manifest or metadata could not be validated.
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
    /// A requested rule acceptor plugin is not registered.
    #[error("missing rule acceptor: {0}")]
    MissingRuleAcceptor(String),
    /// Evaluation yielded, capturing the [`EvalContext`] for later resumption.
    ///
    /// Produced when an `EvalBehavior` rule returns [`crate::EvalError::Yield`].
    /// The host catches this, serializes the context (see
    /// [`EvalContext::to_json`]), and later resumes by re-dispatching `Eval`
    /// with a restored context (see [`crate::BasicEvaluator::resume`]).
    #[error("evaluation yielded")]
    Yield {
        /// The captured execution context — all state needed to resume.
        context: EvalContext,
    },
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
