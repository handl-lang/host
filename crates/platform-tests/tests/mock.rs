use handl_platform::{CapabilityRequest, ExtensionRegistry, Kit, KitRegistry, Plugin, PluginContext, PluginRegistry};
use handl_platform_tests::{MockExtension, MockHost, MockPlugin};

#[test]
fn mock_plugin_implements_trait() {
    let p = MockPlugin::new("fmt", &["fmt", "lint"]).with_output(serde_json::json!({"ok":true}));
    assert_eq!(p.name(), "fmt");
    assert_eq!(p.capabilities().len(), 2);
    let meta = p.metadata(&"fmt".into());
    assert!(meta.is_some());

    let (mut plugins, mut kits, mut extensions) = (PluginRegistry::new(), KitRegistry::new(), ExtensionRegistry::new());
    let mut ctx = PluginContext {
        plugins: &mut plugins,
        kits: &mut kits,
        extensions: &mut extensions,
    };
    let response = p
        .run(&CapabilityRequest {
            capability: "fmt".into(),
            payload: serde_json::Value::Null,
        }, &mut ctx)
        .unwrap();
    assert_eq!(response.0, serde_json::json!({"ok":true}));
}

#[test]
fn mock_host_holds_registries() {
    let mut host = MockHost::new();
    host.plugins.register(MockPlugin::new("compile", &["compile"]));
    host.kits.register(Kit {
        name: "core".into(),
        plugins: vec!["compile".into()],
    });
    host.extensions.register(MockExtension::new(
        "foo",
        serde_json::json!({"name":"foo"}),
    ));

    assert_eq!(host.plugins.capabilities().len(), 1);
    assert_eq!(host.kits.all().len(), 1);
    assert_eq!(host.extensions.all().len(), 1);
}

#[test]
fn mock_plugin_handles_unknown_metadata() {
    let p = MockPlugin::new("one", &["one"]);
    assert!(p.metadata(&"two".into()).is_none());
}
