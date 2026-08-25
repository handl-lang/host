pub mod registry;

pub use registry::LoaderRegistry;

use handl_platform::{PlatformError, Plugin, PluginManifest};

pub trait PluginLoader: Send + Sync {
    fn can_load(&self, manifest: &PluginManifest) -> bool;
    fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin>, PlatformError>;
}
