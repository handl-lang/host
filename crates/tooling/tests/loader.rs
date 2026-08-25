use std::path::PathBuf;

use handl_kit_canonical::CanonicalKit;
use handl_loader::LoaderRegistry;
use handl_loader_native::NativeLoader;
use handl_platform::PluginManifest;
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_tooling::ToolingHost;

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
        "libnative-plugin-example.so",
        "libnative_plugin_example.dylib",
        "libnative-plugin-example.dylib",
        "native_plugin_example.dll",
        "native-plugin-example.dll",
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
fn tooling_host_loads_native_plugin_and_dispatches_command() {
    let Some(cdylib) = find_example_cdylib() else {
        eprintln!("native-plugin-example cdylib not found; skipping loader wiring test");
        return;
    };

    let manifest = PluginManifest {
        id: "native-plugin-example".to_string(),
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

    let mut loaders = LoaderRegistry::new();
    loaders.register(NativeLoader::new());

    let mut host = ToolingHost::new(
        CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    )
    .with_loader_registry(loaders);

    host.load_plugin(&manifest).expect("load native plugin");

    let commands = host.cli_commands();
    assert!(commands.iter().any(|s| s.name == "echo"));

    let output = host.run_cli("echo", &["hello".to_string()]).expect("run echo");
    let json = output.to_string();
    assert!(json.contains("hello"));
}
