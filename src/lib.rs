//! `handl` is the root facade for the Handl language platform.
//!
//! It re-exports the core platform contracts from `handl-platform` and
//! conditionally exposes the major workspace crates behind their Cargo
//! features. Hosts and applications can depend on `handl` with only the
//! features they need.

// Core platform contracts and types (always available).
pub use handl_platform::*;

// Non-optional workspace crates.
pub use handl_ast;
pub use handl_ast_canonical;
pub use handl_ast_common;
pub use handl_dialect;
pub use handl_source;
pub use handl_vm_values_json;

// Optional workspace crates, exposed behind their existing features.
#[cfg(feature = "dialect-canonical")]
pub use handl_dialect_canonical;

#[cfg(feature = "vm")]
pub use handl_vm;

#[cfg(feature = "host")]
pub use handl_vm_host_eval;

#[cfg(feature = "compiler")]
pub use handl_compiler;

#[cfg(feature = "compiler")]
pub use handl_compiler as compiler;

#[cfg(feature = "typecheck")]
pub use handl_typecheck_core;

#[cfg(feature = "typegen")]
pub use handl_typegen_core;

#[cfg(feature = "resolver-core")]
pub use handl_module_resolver_core;

#[cfg(feature = "store")]
pub use handl_store;

#[cfg(feature = "codebase")]
pub use handl_codebase;

#[cfg(test)]
mod tests {
    #[test]
    fn root_reexports_platform_types() {
        let _id = crate::capability::CapabilityId::from("test");
        let _reg = crate::plugin::PluginRegistry::new();
    }

    #[test]
    #[cfg(feature = "compiler")]
    fn root_reexports_optional_when_feature_enabled() {
        // Compile-check that `handl::compiler::Pipeline` is reachable.
        let _: Option<crate::compiler::Pipeline> = None;
    }
}
