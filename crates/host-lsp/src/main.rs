use handl_plugin_ext_lsp::BasicLsp;
use handl_platform::{CapabilityRequest, ConfigHostApplier, DefaultHost, HostApplier, PlatformError};

fn main() -> Result<(), PlatformError> {
    let mut host = DefaultHost::new();
    let applier = ConfigHostApplier::from_toml("")?;
    applier.apply(&mut host)?;
    host.register_plugin(BasicLsp::new());
    host.load()?;
    host.run(&CapabilityRequest {
        capability: "lsp".into(),
        payload: serde_json::Value::Null,
    })?;
    Ok(())
}
