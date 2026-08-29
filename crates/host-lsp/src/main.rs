use handl_plugin_applier::BootstrapPlugin;
use handl_plugin_ext_lsp::BasicLsp;
use handl_platform::{CapabilityRequest, DefaultHost, PlatformError};

fn main() -> Result<(), PlatformError> {
    let mut host = DefaultHost::new();
    host.register_plugin(BootstrapPlugin::new(".handl.toml", "canonical", "applier.default"));
    host.register_plugin(BasicLsp::new());
    host.load()?;
    host.run(&CapabilityRequest {
        capability: "lsp".into(),
        payload: serde_json::Value::Null,
    })?;
    Ok(())
}
