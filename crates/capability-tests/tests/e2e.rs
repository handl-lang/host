use handl_platform::{CapabilityRequest, DefaultHost};
use handl_capability_kit::KitManagerPlugin;
use handl_capability_plugin::PluginManagerPlugin;

#[test]
fn management_capabilities_end_to_end() {
    let mut host = DefaultHost::new();
    host.register_plugin(PluginManagerPlugin::new());
    host.register_plugin(KitManagerPlugin::new());

    let add_plugin = host
        .run(&CapabilityRequest {
            capability: "plugin".into(),
            payload: serde_json::json!({
                "action": "add",
                "name": "remote",
                "capabilities": ["compile"]
            }),
        })
        .unwrap();
    assert_eq!(add_plugin.0, serde_json::json!({"added": "remote"}));

    let info_plugin = host
        .run(&CapabilityRequest {
            capability: "plugin".into(),
            payload: serde_json::json!({"action": "info"}),
        })
        .unwrap();
    let plugin_names = info_plugin.0["plugins"].as_array().unwrap();
    assert!(plugin_names.iter().any(|v| v == "remote"));

    let add_kit = host
        .run(&CapabilityRequest {
            capability: "kit".into(),
            payload: serde_json::json!({
                "action": "add",
                "name": "core",
                "plugins": ["fmt", "lint"]
            }),
        })
        .unwrap();
    assert_eq!(add_kit.0, serde_json::json!({"added": "core"}));

    let info_kit = host
        .run(&CapabilityRequest {
            capability: "kit".into(),
            payload: serde_json::json!({"action": "info"}),
        })
        .unwrap();
    assert_eq!(info_kit.0["kits"], serde_json::json!(["core"]));
}
