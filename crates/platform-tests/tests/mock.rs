//! `Plugin` trait surface and registry dispatch, driven through the mock.

use handl_platform::{DefaultHost, PlatformError, Plugin, PluginRegistry};
use handl_platform_tests::{MockPlugin, request};

#[test]
fn mock_plugin_exposes_name_capabilities_and_metadata() {
    let plugin =
        MockPlugin::new("fmt", &["fmt", "lint"]).with_output(serde_json::json!({"ok": true}));

    assert_eq!(plugin.name(), "fmt");
    assert_eq!(plugin.id(), "fmt");
    assert_eq!(plugin.capabilities().len(), 2);
    assert!(plugin.metadata(&"lint".into()).is_some());
    assert!(plugin.metadata(&"absent".into()).is_none());
    assert!(plugin.calls().is_empty(), "no request served yet");
}

#[test]
fn registry_reports_the_mock_capabilities_with_metadata() {
    let mut plugins = PluginRegistry::new();
    plugins.register(MockPlugin::new("fmt", &["fmt", "lint"]));

    let caps = plugins.capabilities();
    assert_eq!(caps.len(), 2);

    let mut ids: Vec<_> = caps.iter().map(|(id, _)| id.as_str().to_string()).collect();
    ids.sort();
    assert_eq!(ids, ["fmt", "lint"]);

    let (_, meta) = caps
        .iter()
        .find(|(id, _)| id.as_str() == "fmt")
        .expect("fmt is registered");
    assert_eq!(meta["plugin"], "fmt");
}

#[test]
fn dispatch_through_the_host_reaches_the_mock() {
    let plugin = MockPlugin::new("fmt", &["fmt"]).with_output(serde_json::json!({"status": "ok"}));
    let log = plugin.clone();

    let mut host = DefaultHost::new();
    host.register_plugin(plugin);

    let response = host
        .run(&request("fmt", serde_json::json!({"src": "x"})))
        .expect("fmt is provided");
    assert_eq!(response.0, serde_json::json!({"status": "ok"}));
    assert_eq!(log.calls(), ["fmt"], "the mock served exactly one request");
}

#[test]
fn unknown_capability_is_reported_and_logged_nowhere() {
    let plugin = MockPlugin::new("fmt", &["fmt"]);
    let log = plugin.clone();

    let mut host = DefaultHost::new();
    host.register_plugin(plugin);

    assert!(matches!(
        host.run(&request("absent", serde_json::Value::Null)),
        Err(PlatformError::UnknownCapability(_))
    ));
    assert!(log.calls().is_empty());
}
