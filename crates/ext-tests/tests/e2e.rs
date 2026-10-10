//! Restored end-to-end test for the extension management capabilities.
//!
//! This is the pre-`bcf31afa` `crates/ext-tests/tests/e2e.rs` (deleted with the
//! "obsolete crates" sweep) restored against today's crates. The test body is
//! unchanged — both plugin types and their `{action, name, …}` payload
//! contracts survived the crate renames (`handl-ext-{kit,plugin}` →
//! `plugin-ext-{kit,plugin}`), so only the `use` paths moved. That is worth
//! stating because it is the exception: the sibling files from that sweep
//! (`integration-tests/*`, `platform-tests/language_host.rs`) targeted
//! `handl-typecheck-*`, `handl-ast-canonical`, and platform's removed
//! `LanguageHost`/`SyntaxNode`, and cannot be restored without a rewrite.

use handl_platform::{CapabilityRequest, DefaultHost};
use handl_plugin_ext_kit::KitManagerPlugin;
use handl_plugin_ext_plugin::PluginManagerPlugin;

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
