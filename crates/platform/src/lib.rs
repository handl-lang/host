//! The `handl-platform` crate provides shared platform abstractions for the
//! HAndL language toolchain, including plugins, capabilities, manifests,
//! resolution, hosts, and boundaries.
#![warn(missing_docs)]

/// Types for describing runtime isolation boundaries.
pub mod boundary;
/// Generic build operation and rule acceptor.
pub mod build;
/// Capability identifiers and metadata.
pub mod capability;
/// Generic compile operation and rule acceptor.
pub mod compile;
/// Host configuration and appliers.
pub mod config;
/// Platform error types.
pub mod error;
/// Generic evaluation operation and rule acceptor.
pub mod eval;
/// Generic format operation and rule acceptor.
pub mod format;
/// Expression formatter trait for formatting concrete VM expression trees.
pub mod formatter;
/// Host implementations that coordinate plugins and kits.
pub mod host;
/// Kit manifests and registries.
pub mod kit;
/// Generic lint operation and rule acceptor.
pub mod lint;
/// Plugin manifest definitions and dependency declarations.
pub mod manifest;
/// Generic, composable AST node.
pub mod ast_node;
/// Plugin-provided option schemas and resolved values.
pub mod options;
/// Generic typecheck operation and rule acceptor.
pub mod typecheck;
/// Foundational composable plugin types.
pub mod plugin_model;
/// Plugin traits, registries, and built-in plugin implementations.
pub mod plugin;
/// Plugin composition resolution.
pub mod resolve;
/// Runtime built-in host and behavior rules.
pub mod runtime_host;
/// Unified `Value`/`Term` space for the platform VM.
pub mod value;
/// Composable VM wrapper over the unified value space.
pub mod vm;

pub use boundary::*;
pub use build::*;
pub use capability::*;
pub use compile::*;
pub use config::*;
pub use error::*;
pub use eval::*;
pub use format::*;
pub use formatter::*;
pub use host::*;
pub use kit::*;
pub use lint::*;
pub use manifest::*;
pub use ast_node::*;
pub use options::*;
pub use typecheck::*;
pub use plugin_model::*;
pub use plugin::*;
pub use resolve::*;
pub use runtime_host::*;

/// Unified value space primitives re-exported from `handl_plugin_lib_type`.
pub use value::{Env, Term, Value, prelude_env, prelude_tctx, typecheck, typecheck_with_prelude};
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
