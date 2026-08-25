pub mod boundary;
pub mod capability;
pub mod error;
pub mod extension;
pub mod host;
pub mod kit;
pub mod plugin;

pub use boundary::*;
pub use capability::*;
pub use error::*;
pub use extension::*;
pub use host::*;
pub use kit::*;
pub use plugin::*;

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
