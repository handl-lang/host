//! The `handl-platform` crate provides shared platform abstractions for the
//! HAndL language toolchain, including plugins, capabilities, manifests,
//! resolution, hosts, and boundaries.
#![warn(missing_docs)]

/// Generic, composable AST node.
pub mod ast_node;
/// Types for describing runtime isolation boundaries.
pub mod boundary;
/// Generic build operation and rule acceptor.
pub mod build;
/// Capability identifiers and metadata.
pub mod capability;
/// Generic compile operation and rule acceptor.
pub mod compile;
/// Runtime configuration and appliers.
pub mod config;
/// Platform error types.
pub mod error;
/// Generic evaluation operation and rule acceptor.
pub mod eval;
/// Generic format operation and rule acceptor.
pub mod format;
/// Runtime implementations that coordinate plugins and kits.
pub mod host;
/// Kit manifests and registries.
pub mod kit;
/// Generic lint operation and rule acceptor.
pub mod lint;
/// Plugin manifest definitions and dependency declarations.
pub mod manifest;
/// Plugin-provided option schemas and resolved values.
pub mod options;
/// Plugin traits, registries, and built-in plugin implementations.
pub mod plugin;
/// Foundational composable plugin types.
pub mod plugin_model;
/// Plugin composition resolution.
pub mod resolve;
/// Composable VM wrapper over the unified value space.
pub mod vm;

pub use ast_node::*;
pub use boundary::*;
pub use build::*;
pub use capability::*;
pub use compile::*;
pub use config::*;
pub use error::*;
pub use eval::*;
pub use format::*;
pub use host::*;
pub use kit::*;
pub use lint::*;
pub use manifest::*;
pub use options::*;
pub use plugin::*;
pub use plugin_model::*;
pub use resolve::*;
/// Composable VM wrapper.
pub use vm::Vm;

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
