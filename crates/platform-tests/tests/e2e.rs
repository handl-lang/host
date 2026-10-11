//! End-to-end: a simulated host loads a kit and dispatches the kit's declared
//! capabilities.
//!
//! Restored from the pre-composable `platform-tests/tests/e2e.rs`, which used
//! the crate's own `MockHost`; that role is now played by
//! [`handl_platform::DefaultHost`], so this test asserts against the host the
//! platform actually ships rather than a test double of it.

use handl_platform::{CallableLookup, DefaultHost, Kit, KitManifest};
use handl_platform_tests::{MockPlugin, mock_manifest, request};

#[test]
fn simulated_host_loads_kit_and_dispatches() {
    let fmt = MockPlugin::new("fmt", &["fmt"]).with_output(serde_json::json!({"status": "ok"}));
    let lint = MockPlugin::new("lint", &["lint"]).with_output(serde_json::json!({"status": "ok"}));

    let mut host = DefaultHost::new();
    host.register_plugin(fmt.clone());
    host.register_plugin(lint.clone());

    host.register_kit(Kit {
        manifest: KitManifest {
            id: "core".into(),
            name: "core".into(),
            plugins: vec!["fmt".into(), "lint".into()],
            target: None,
            selectors: Vec::new(),
        },
        available_plugins: vec![
            mock_manifest("fmt", &["fmt"]),
            mock_manifest("lint", &["lint"]),
        ],
    });
    host.load().expect("host loads to a fixed point");

    let kit = host.kits().resolve("core").expect("kit is loaded");
    assert_eq!(kit.manifest.name, "core");
    assert_eq!(kit.manifest.plugins.len(), 2);

    // Every capability the kit declares is served by a registered plugin.
    for cap in ["fmt", "lint"] {
        let response = host
            .run(&request(cap, serde_json::json!({"src": "x"})))
            .unwrap_or_else(|e| panic!("{cap} should be provided: {e}"));
        assert_eq!(response.0, serde_json::json!({"status": "ok"}));
    }

    assert_eq!(fmt.calls(), ["fmt"]);
    assert_eq!(lint.calls(), ["lint"]);
}
