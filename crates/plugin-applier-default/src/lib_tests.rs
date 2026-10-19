use super::*;
use handl_kit_data::LibraryRegistry;
use handl_platform::{CallableLookup, DefaultHost};
use handl_plugin_load::LoaderHostPlugin;
use std::fs;
use std::sync::Arc;

/// The canonical kit-manifest resolver for tests — assembled from the
/// dev-dep data crate (tests may depend on `lang/` data; the applier must
/// not).
fn kit_module_names() -> KitModuleNames {
    Arc::new(|name: &str| {
        handl_kit_data_prelude::kit_module_names(name).map_err(|e| e.to_string())
    })
}

/// The canonical library-module table for tests — supplied by the kit
/// (ADR-0128 §0.4), not by the loader.
fn libraries() -> Arc<LibraryRegistry> {
    handl_kit_prelude::prelude_library_registry()
}

#[test]
fn applier_loads_kit_from_config() {
    let mut path = std::env::temp_dir();
    path.push("applier_test.handl.toml");
    fs::write(&path, "[[kit]]\ninclude = \"hello\"\n").unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new(kit_module_names(), libraries()));
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(path.as_path().to_str().unwrap(), kit_module_names(), libraries()).with_fallback_kit("canonical"),
    );
    host.load().unwrap();

    let names = host.plugins().names();
    assert!(names.iter().any(|n| n == "lib-hello"), "names: {names:?}");

    let _ = fs::remove_file(&path);
}

#[test]
fn full_config_flow_kits_plugins_options() {
    let mut config_path = std::env::temp_dir();
    config_path.push("full_flow.handl.toml");
    fs::write(
        &config_path,
        r#"
[[kit]]
include = "hello"

[[plugin]]
register = "lib.hello"

[options]
"comment.style" = "above"
"#,
    )
    .unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new(kit_module_names(), libraries()));
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap(), kit_module_names(), libraries())
            .with_fallback_kit("canonical"),
    );
    host.load().unwrap();

    let names = host.plugins().names();
    assert!(names.iter().any(|n| n == "lib-hello"), "names: {names:?}");
    assert!(names.iter().any(|n| n == "options"), "names: {names:?}");

    let _ = fs::remove_file(&config_path);
}

#[test]
fn register_accepts_unresolved_declaration() {
    let mut config_path = std::env::temp_dir();
    config_path.push("missing_plugin.handl.toml");
    fs::write(
        &config_path,
        r#"
[[plugin]]
register = "nonexistent.plugin.id"
"#,
    )
    .unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new(kit_module_names(), libraries()));
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap(), kit_module_names(), libraries())
            .with_fallback_kit("hello"),
    );
    let result = host.load();
    assert!(
        result.is_ok(),
        "register is a declaration, not an immediate check: {result:?}"
    );

    let _ = fs::remove_file(&config_path);
}

#[test]
fn excludes_remove_plugin_by_id() {
    let mut config_path = std::env::temp_dir();
    config_path.push("excludes_test.handl.toml");
    fs::write(
        &config_path,
        r#"
[[kit]]
include = "hello"

exclude = ["lib.hello"]
"#,
    )
    .unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new(kit_module_names(), libraries()));
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap(), kit_module_names(), libraries())
            .with_fallback_kit("canonical"),
    );
    host.load().unwrap();

    let ids = host.plugins().ids();
    assert!(
        !ids.iter().any(|id| id == "lib.hello"),
        "lib.hello should be excluded: {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id == "kit.hello"),
        "kit.hello should remain: {ids:?}"
    );

    let _ = fs::remove_file(&config_path);
}

#[test]
fn per_kit_exclude_removes_plugin() {
    let mut config_path = std::env::temp_dir();
    config_path.push("kit_exclude_test.handl.toml");
    fs::write(
        &config_path,
        r#"
[[kit]]
include = "hello"
exclude = ["lib.hello"]
"#,
    )
    .unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new(kit_module_names(), libraries()));
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap(), kit_module_names(), libraries())
            .with_fallback_kit("canonical"),
    );
    host.load().unwrap();

    let ids = host.plugins().ids();
    assert!(
        !ids.iter().any(|id| id == "lib.hello"),
        "lib.hello should be excluded: {ids:?}"
    );

    let _ = fs::remove_file(&config_path);
}
