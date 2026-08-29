use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;

use handl_plugin_load::{LoaderHostPlugin, LoaderRule, PluginLoader};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, ContributionContext, PlatformError, Plugin,
    PluginContext, PluginManifest,
};

#[derive(Clone, Default)]
pub struct NativeLoader;

impl NativeLoader {
    pub fn new() -> Self {
        Self
    }

    fn resolve_path(&self, manifest: &PluginManifest) -> Option<PathBuf> {
        if let Some(path) = manifest
            .metadata
            .get("path")
            .and_then(|value| value.as_str())
        {
            return Some(PathBuf::from(path));
        }

        let id = &manifest.id;
        if id.ends_with(".so") || id.ends_with(".dylib") || id.ends_with(".dll") {
            Some(PathBuf::from(id))
        } else {
            None
        }
    }
}

impl PluginLoader for NativeLoader {
    fn can_load(&self, manifest: &PluginManifest) -> bool {
        self.resolve_path(manifest).is_some()
    }

    fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin + 'static>, PlatformError> {
        let path = self.resolve_path(manifest).ok_or_else(|| {
            PlatformError::Plugin("manifest has no native library path".to_string())
        })?;

        let lib = unsafe { libloading::Library::new(&path) }
            .map_err(|e| PlatformError::Plugin(format!("failed to load library {path:?}: {e}")))?;

        let run: libloading::Symbol<unsafe extern "C" fn(*const c_char) -> *const c_char> =
            unsafe { lib.get(b"handl_plugin_run\0") }.map_err(|e| {
                PlatformError::Plugin(format!("missing handl_plugin_run symbol: {e}"))
            })?;
        let run_fn = *run;

        Ok(Box::new(SharedPlugin {
            manifest: manifest.clone(),
            _library: lib,
            run: run_fn,
        }))
    }
}

impl Plugin for NativeLoader {
    fn name(&self) -> &str {
        "loader-native"
    }

    fn id(&self) -> &str {
        "loader.native"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if ctx.get_plugin_mut::<LoaderHostPlugin>().is_none() {
            ctx.register_plugin(LoaderHostPlugin::default())?;
        }
        ctx.contribute::<LoaderHostPlugin>(LoaderRule {
            id: self.name().to_string(),
            loader: Box::new(self.clone()),
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::UnknownCapability(request.capability.clone()))
    }
}

struct SharedPlugin {
    manifest: PluginManifest,
    _library: libloading::Library,
    run: unsafe extern "C" fn(*const c_char) -> *const c_char,
}

impl Plugin for SharedPlugin {
    fn name(&self) -> &str {
        &self.manifest.id
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.manifest.provides
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<serde_json::Value> {
        self.manifest
            .provides
            .contains(capability)
            .then(|| self.manifest.metadata.clone())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let request_json = serde_json::to_string(request)
            .map_err(|e| PlatformError::Plugin(format!("failed to serialize request: {e}")))?;
        let c_request = CString::new(request_json)
            .map_err(|e| PlatformError::Plugin(format!("request contains null byte: {e}")))?;

        let result = unsafe { (self.run)(c_request.as_ptr()) };

        if result.is_null() {
            return Err(PlatformError::Plugin("native plugin returned null".to_string()));
        }

        let response = unsafe { CStr::from_ptr(result) };
        let response_str = response
            .to_str()
            .map_err(|e| PlatformError::Plugin(format!("non-utf8 response: {e}")))?;
        let value = serde_json::from_str(response_str)
            .map_err(|e| PlatformError::Plugin(format!("invalid plugin response: {e}")))?;

        Ok(CapabilityResponse(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_path_returns_error() {
        let manifest = PluginManifest {
            id: "missing".to_string(),
            version: "0.1.0".to_string(),
            provides: vec!["echo".into()],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::json!({"path": "/no/such/library.so"}),
        };

        let loader = NativeLoader::new();
        assert!(loader.can_load(&manifest));
        assert!(loader.load(&manifest).is_err());
    }

    fn target_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
    }

    fn find_example_cdylib() -> Option<PathBuf> {
        let dir = target_dir();
        if !dir.is_dir() {
            return None;
        }

        let candidates: &[&str] = &[
            "libnative_plugin_example.so",
            "libhandl-plugin-target-native-example.so",
            "libnative_plugin_example.dylib",
            "libhandl-plugin-target-native-example.dylib",
            "native_plugin_example.dll",
            "handl-plugin-target-native-example.dll",
        ];

        for name in candidates {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
        }

        None
    }

    #[test]
    fn load_native_example_echo() {
        let Some(cdylib) = find_example_cdylib() else {
            eprintln!("handl-plugin-target-native-example cdylib not found; skipping load test");
            return;
        };

        let manifest = PluginManifest {
            id: "handl-plugin-target-native-example".to_string(),
            version: "0.1.0".to_string(),
            provides: vec!["echo".into()],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::json!({"path": cdylib.to_string_lossy().as_ref()}),
        };

        let loader = NativeLoader::new();
        let plugin = loader.load(&manifest).expect("load example cdylib");
        assert!(plugin.capabilities().contains(&CapabilityId::from("echo")));

        let (mut plugins, mut kits) = (
            handl_platform::PluginRegistry::new(),
            handl_platform::KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };

        let response = plugin
            .run(
                &CapabilityRequest {
                    capability: "echo".into(),
                    payload: serde_json::json!({"hello": "world"}),
                },
                &mut ctx,
            )
            .expect("echo request");

        assert_eq!(response.0, serde_json::json!({"hello": "world"}));
    }
}
