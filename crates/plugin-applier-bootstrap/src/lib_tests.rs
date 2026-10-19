use super::*;
use handl_kit_data::LibraryRegistry;
use handl_platform::{CallableLookup, DefaultHost};
use std::fs;
use std::sync::Arc;

/// The canonical kit-manifest resolver for tests — assembled from the
/// dev-dep data crate.
fn kit_module_names() -> KitModuleNames {
    Arc::new(|name: &str| {
        handl_kit_data_prelude::kit_module_names(name).map_err(|e| e.to_string())
    })
}

/// The canonical library-module table for tests (ADR-0128 §0.4).
fn libraries() -> Arc<LibraryRegistry> {
    handl_kit_prelude::prelude_library_registry()
}

#[test]
fn bootstrap_loads_kit_via_applier() {
    let mut path = std::env::temp_dir();
    path.push("bootstrap_test.handl.toml");
    fs::write(&path, "[[kit]]\ninclude = \"hello\"\n").unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BootstrapPlugin::new(
        path.as_path().to_str().unwrap(),
        "canonical",
        "applier.default",
        kit_module_names(),
        libraries(),
    ));
    host.load().unwrap();

    let names = host.plugins().names();
    assert!(names.iter().any(|n| n == "lib-hello"), "names: {names:?}");
    assert!(
        names.iter().any(|n| n == "default-applier"),
        "names: {names:?}"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn bootstrap_rejects_unknown_applier() {
    let mut host = DefaultHost::new();
    host.register_plugin(BootstrapPlugin::new(
        ".handl.toml",
        "hello",
        "applier.custom",
        kit_module_names(),
        libraries(),
    ));
    let result = host.load();
    assert!(result.is_err());
}
