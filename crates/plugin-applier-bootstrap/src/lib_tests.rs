use super::*;
use handl_platform::{CallableLookup, DefaultHost};
use std::fs;

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
    ));
    let result = host.load();
    assert!(result.is_err());
}
