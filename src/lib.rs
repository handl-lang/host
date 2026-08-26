//! `handl` is the root facade for the Handl language platform.
//!
//! It re-exports the core platform contracts from `handl-platform` and
//! the generic base crates that survive the canonical-kit retirement.

// Core platform contracts and types (always available).
pub use handl_platform::*;

// Non-optional workspace crates that remain after the canonical retirement.
pub use handl_ast;
pub use handl_source;

#[cfg(test)]
mod tests {
    #[test]
    fn root_reexports_platform_types() {
        let _id = crate::capability::CapabilityId::from("test");
        let _reg = crate::plugin::PluginRegistry::new();
    }
}
