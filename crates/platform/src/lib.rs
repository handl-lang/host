//! The `handl-platform` crate provides shared platform abstractions for the
//! HAndL language toolchain, including plugins, capabilities, manifests,
//! resolution, language pipelines, hosts, and boundaries.
#![warn(missing_docs)]

/// Types for describing runtime isolation boundaries.
pub mod boundary;
/// Capability identifiers and metadata.
pub mod capability;
/// Host configuration and appliers.
pub mod config;
/// Platform error types.
pub mod error;
/// Extension registration and discovery.
pub mod extension;
/// Host implementations that coordinate plugins, kits, and extensions.
pub mod host;
/// Kit manifests and registries.
pub mod kit;
/// Language pipeline plugins and syntax types.
pub mod language;
/// Plugin manifest definitions and dependency declarations.
pub mod manifest;
/// Generic, composable AST node for ADR-0002.
pub mod ast_node;
/// Plugin-provided option schemas and resolved values.
pub mod options;
/// Foundational composable plugin types for ADR-0002.
pub mod plugin_model;
/// Plugin traits, registries, and built-in plugin implementations.
pub mod plugin;
/// Plugin composition resolution.
pub mod resolve;

pub use boundary::*;
pub use capability::*;
pub use config::*;
pub use error::*;
pub use extension::*;
pub use host::*;
pub use kit::*;
pub use language::*;
pub use manifest::*;
pub use ast_node::*;
pub use options::*;
pub use plugin_model::*;
pub use plugin::*;
pub use resolve::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_reexports_are_usable() {
        let id = CapabilityId::from("test");
        assert_eq!(id.as_str(), "test");
        let _reg = PluginRegistry::new();
        let _host = DefaultHost::new();
    }
}
