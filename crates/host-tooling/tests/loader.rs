use std::path::PathBuf;

use handl_platform::{CapabilityRequest, DefaultHost, PlatformError, PluginManifest};
use handl_plugin_loader_native::NativeLoader;

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
#[ignore = "loader/cli wiring not yet implemented"]
fn tooling_host_loads_native_plugin_and_dispatches_command() -> Result<(), PlatformError> {
    let Some(cdylib) = find_example_cdylib() else {
        eprintln!("handl-plugin-target-native-example cdylib not found; skipping loader wiring test");
        return Ok(());
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
        metadata: serde_json::json!({
            "path": cdylib.to_string_lossy().as_ref(),
            "command": "echo",
            "description": "Echo plugin"
        }),
    };

    let mut host = DefaultHost::new();
    host.register_plugin(NativeLoader::new());
    host.load()?;

    let payload = serde_json::to_value(&manifest).expect("serialize manifest");
    host.run(&CapabilityRequest {
        capability: "load".into(),
        payload,
    })?;

    let response = host.run(&CapabilityRequest {
        capability: "echo".into(),
        payload: serde_json::json!("hello"),
    })?;
    assert!(response.0.to_string().contains("hello"));

    Ok(())
}
