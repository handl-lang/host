//! `handl` is the root facade for the Handl language platform.
//!
//! It re-exports the core platform contracts from `handl-platform`.

// Core platform contracts and types (always available).
pub use handl_platform::*;

#[cfg(test)]
mod tests {
    #[test]
    fn root_reexports_platform_types() {
        let _id = crate::capability::CapabilityId::from("test");
        let _reg = crate::plugin::PluginRegistry::new();
    }
}
