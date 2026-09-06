use super::*;
use handl_platform::{CallableLookup, DefaultHost};
use handl_plugin_load::LoaderHostPlugin;
use std::fs;

#[test]
fn applier_loads_kit_from_config() {
    let mut path = std::env::temp_dir();
    path.push("applier_test.handl.toml");
    fs::write(&path, "[[kit]]\ninclude = \"hello\"\n").unwrap();

    let mut host = DefaultHost::new();
    host.register_plugin(BasicKitLoaderPlugin::new());
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(path.as_path().to_str().unwrap()).with_fallback_kit("canonical"),
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
    host.register_plugin(BasicKitLoaderPlugin::new());
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap())
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
    host.register_plugin(BasicKitLoaderPlugin::new());
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap())
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
    host.register_plugin(BasicKitLoaderPlugin::new());
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap())
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
    host.register_plugin(BasicKitLoaderPlugin::new());
    host.register_plugin(LoaderHostPlugin::default());
    host.register_plugin(
        DefaultApplierPlugin::new(config_path.as_path().to_str().unwrap())
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
