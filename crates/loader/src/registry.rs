use std::sync::Arc;

use handl_platform::{PlatformError, Plugin, PluginManifest};

use crate::PluginLoader;

#[derive(Default)]
pub struct LoaderRegistry {
    loaders: Vec<Arc<dyn PluginLoader>>,
}

impl LoaderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, loader: impl PluginLoader + 'static) {
        self.loaders.push(Arc::new(loader));
    }

    pub fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin>, PlatformError> {
        for loader in &self.loaders {
            if !loader.can_load(manifest) {
                continue;
            }
            match loader.load(manifest) {
                Ok(plugin) => return Ok(plugin),
                Err(PlatformError::NotImplemented) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(PlatformError::Plugin("no loader could load manifest".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{plugin::InProcessPlugin, CapabilityId, CapabilityRequest, CapabilityResponse};

    struct AcceptLoader {
        prefix: String,
        name: String,
    }

    impl PluginLoader for AcceptLoader {
        fn can_load(&self, manifest: &PluginManifest) -> bool {
            manifest.id.starts_with(&self.prefix)
        }

        fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin>, PlatformError> {
            Ok(Box::new(InProcessPlugin::new(
                manifest.id.clone(),
                &[CapabilityId::from(self.name.as_str())],
                serde_json::Value::Null,
                |_req: &CapabilityRequest, _ctx: &mut handl_platform::PluginContext| {
                    Ok(CapabilityResponse(serde_json::Value::Null))
                },
            )))
        }
    }

    #[test]
    fn loader_registry_tries_in_order() {
        let mut reg = LoaderRegistry::new();
        reg.register(AcceptLoader {
            prefix: "a".to_string(),
            name: "a".to_string(),
        });
        reg.register(AcceptLoader {
            prefix: "o".to_string(),
            name: "fallback".to_string(),
        });

        let alpha = PluginManifest {
            id: "alpha".to_string(),
            version: "0.1.0".to_string(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::default(),
        };
        let plugin = reg.load(&alpha).unwrap();
        assert_eq!(plugin.name(), "alpha");
        assert!(plugin.capabilities().iter().any(|c| c.as_str() == "a"));

        let other = PluginManifest {
            id: "other".to_string(),
            ..alpha.clone()
        };
        let plugin = reg.load(&other).unwrap();
        assert_eq!(plugin.name(), "other");
        assert!(plugin.capabilities().iter().any(|c| c.as_str() == "fallback"));

        assert!(reg.load(&PluginManifest {
            id: "no".to_string(),
            ..alpha
        }).is_err());
    }

    #[test]
    fn loader_registry_loads_native_wasm_web_in_order() {
        let mut reg = LoaderRegistry::new();
        reg.register(AcceptLoader {
            prefix: "native".to_string(),
            name: "native".to_string(),
        });
        reg.register(AcceptLoader {
            prefix: "wasm".to_string(),
            name: "wasm".to_string(),
        });
        reg.register(AcceptLoader {
            prefix: "http".to_string(),
            name: "web".to_string(),
        });

        let base = PluginManifest {
            id: "base".to_string(),
            version: "0.1.0".to_string(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::default(),
        };

        let native = PluginManifest {
            id: "native://foo".to_string(),
            ..base.clone()
        };
        let plugin = reg.load(&native).unwrap();
        assert!(plugin.capabilities().iter().any(|c| c.as_str() == "native"));

        let wasm = PluginManifest {
            id: "wasm://bar".to_string(),
            ..base.clone()
        };
        let plugin = reg.load(&wasm).unwrap();
        assert!(plugin.capabilities().iter().any(|c| c.as_str() == "wasm"));

        let web = PluginManifest {
            id: "http://example.com".to_string(),
            ..base
        };
        let plugin = reg.load(&web).unwrap();
        assert!(plugin.capabilities().iter().any(|c| c.as_str() == "web"));
    }
}
