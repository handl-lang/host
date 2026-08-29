use std::fs;
use std::path::PathBuf;

use handl_plugin_load::{LoaderHostPlugin, LoaderRule, PluginLoader};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, ContributionContext, PlatformError, Plugin,
    PluginContext, PluginManifest,
};

#[derive(Clone, Default)]
pub struct WasmLoader;

impl WasmLoader {
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

        if manifest.id.ends_with(".wasm") {
            Some(PathBuf::from(&manifest.id))
        } else {
            None
        }
    }
}

impl PluginLoader for WasmLoader {
    fn can_load(&self, manifest: &PluginManifest) -> bool {
        self.resolve_path(manifest).is_some()
    }

    fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin + 'static>, PlatformError> {
        let path = self
            .resolve_path(manifest)
            .ok_or_else(|| PlatformError::Plugin("manifest has no wasm path".to_string()))?;

        let wasm = fs::read(&path)
            .map_err(|e| PlatformError::Plugin(format!("failed to read wasm {path:?}: {e}")))?;

        Ok(Box::new(SharedWasmPlugin {
            manifest: manifest.clone(),
            wasm,
        }))
    }
}

impl Plugin for WasmLoader {
    fn name(&self) -> &str {
        "loader-wasm"
    }

    fn id(&self) -> &str {
        "loader.wasm"
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

struct SharedWasmPlugin {
    manifest: PluginManifest,
    wasm: Vec<u8>,
}

impl Plugin for SharedWasmPlugin {
    fn name(&self) -> &str {
        &self.manifest.id
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.manifest.provides
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        None
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let request_json = serde_json::to_string(request)
            .map_err(|e| PlatformError::Plugin(format!("failed to serialize request: {e}")))?;

        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, &self.wasm[..])
            .map_err(|e| PlatformError::Plugin(format!("failed to compile wasm: {e}")))?;

        let mut store = wasmi::Store::new(&engine, ());
        let linker = wasmi::Linker::<()>::new(&engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| PlatformError::Plugin(format!("failed to instantiate wasm: {e}")))?
            .start(&mut store)
            .map_err(|e| PlatformError::Plugin(format!("failed to start wasm: {e}")))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| PlatformError::Plugin("wasm module does not export memory".to_string()))?;

        let run = instance
            .get_typed_func::<(i32, i32), i32>(&store, "handl_plugin_run")
            .map_err(|e| PlatformError::Plugin(format!("missing handl_plugin_run export: {e}")))?;

        const INPUT_PTR: u32 = 64;
        let input_len = request_json.len();
        memory
            .write(&mut store, INPUT_PTR as usize, request_json.as_bytes())
            .map_err(|e| PlatformError::Plugin(format!("failed to write request to wasm memory: {e}")))?;

        let out_ptr = run
            .call(&mut store, (INPUT_PTR as i32, input_len as i32))
            .map_err(|e| PlatformError::Plugin(format!("wasm handl_plugin_run failed: {e}")))? as usize;

        let mem_size = memory.current_pages(&store).to_bytes().unwrap_or(0);
        if out_ptr >= mem_size {
            return Err(PlatformError::Plugin("wasm returned invalid output pointer".to_string()));
        }

        let mut buf = vec![0u8; mem_size - out_ptr];
        memory
            .read(&store, out_ptr, &mut buf)
            .map_err(|e| PlatformError::Plugin(format!("failed to read wasm response: {e}")))?;

        let null_pos = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let response_str = std::str::from_utf8(&buf[..null_pos])
            .map_err(|e| PlatformError::Plugin(format!("non-utf8 response: {e}")))?;
        let value = serde_json::from_str(response_str)
            .map_err(|e| PlatformError::Plugin(format!("invalid plugin response: {e}")))?;

        Ok(CapabilityResponse(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_wasm_echo_fixture() {
        let wat_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/echo.wat");
        let wasm_bytes = wat::parse_file(&wat_path).expect("parse tests/echo.wat");

        let wasm_path = std::env::temp_dir().join("handl_wasm_echo_test.wasm");
        std::fs::write(&wasm_path, wasm_bytes).expect("write temporary wasm");

        let manifest = PluginManifest {
            id: "wasm-echo".to_string(),
            version: "0.1.0".to_string(),
            provides: vec!["echo".into()],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::json!({
                "path": wasm_path.to_string_lossy().as_ref()
            }),
        };

        let loader = WasmLoader::new();
        assert!(loader.can_load(&manifest));
        let plugin = loader.load(&manifest).expect("load wasm");
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
            .expect("run wasm echo");

        assert_eq!(response.0["payload"]["hello"], "world");
    }
}
