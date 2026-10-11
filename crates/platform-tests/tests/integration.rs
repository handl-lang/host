//! Registry + kit + host integration: several plugins on one host, a kit
//! resolved by name, every capability dispatched, and the composition
//! validated by the manifest-level resolver.

use handl_platform::{CallableLookup, DefaultHost, Kit, KitManifest, PlatformError, Plugin};
use handl_platform_tests::{MockPlugin, mock_manifest, request};

/// A host with the three capabilities a minimal front end needs, plus the
/// logs of what each mock served.
fn core_host() -> (DefaultHost, Vec<MockPlugin>) {
    let fmt =
        MockPlugin::new("fmt", &["fmt"]).with_output(serde_json::json!({"status": "formatted"}));
    let lint =
        MockPlugin::new("lint", &["lint"]).with_output(serde_json::json!({"status": "linted"}));
    let echo =
        MockPlugin::new("echo", &["echo"]).with_output(serde_json::json!({"status": "echoed"}));
    let logs = vec![fmt.clone(), lint.clone(), echo.clone()];

    let mut host = DefaultHost::new();
    host.register_plugin(fmt);
    host.register_plugin(lint);
    host.register_plugin(echo);
    (host, logs)
}

#[test]
fn host_reports_every_registered_capability() {
    let (mut host, _logs) = core_host();
    host.load().expect("host loads to a fixed point");

    let mut ids: Vec<_> = host
        .plugins()
        .capabilities()
        .into_iter()
        .map(|(id, _)| id.as_str().to_string())
        .collect();
    ids.sort();
    assert_eq!(ids, ["echo", "fmt", "lint"]);
}

#[test]
fn every_capability_dispatches_to_its_own_plugin() {
    let (mut host, logs) = core_host();
    host.load().expect("host loads to a fixed point");

    for cap in ["fmt", "lint", "echo"] {
        let response = host
            .run(&request(cap, serde_json::json!({"src": "1"})))
            .unwrap_or_else(|e| panic!("{cap} should be provided: {e}"));
        assert!(response.0.get("status").is_some());
    }

    for (plugin, expected) in logs.iter().zip(["fmt", "lint", "echo"]) {
        assert_eq!(
            plugin.calls(),
            [expected],
            "{} served {}",
            plugin.name(),
            expected
        );
    }
}

#[test]
fn kit_resolves_by_name_and_composes_its_selected_plugins() {
    let (mut host, _logs) = core_host();
    host.load().expect("host loads to a fixed point");

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
            mock_manifest("echo", &["echo"]),
        ],
    });

    let kit = host
        .kits()
        .resolve("core")
        .expect("kit is registered under its manifest name");
    assert_eq!(kit.manifest.plugins, ["fmt", "lint"]);
    assert!(host.kits().resolve("absent").is_none());

    let composition = kit.resolve().expect("composition resolves");
    let mut selected = composition.selected_plugins.clone();
    selected.sort();
    assert_eq!(selected, ["fmt", "lint"]);
    assert_eq!(
        composition
            .provider_map
            .get(&"fmt".into())
            .map(String::as_str),
        Some("fmt")
    );
}

#[test]
fn unknown_capability_is_an_error_at_the_host_boundary() {
    let (mut host, _logs) = core_host();
    host.load().expect("host loads to a fixed point");

    assert!(matches!(
        host.run(&request("absent", serde_json::Value::Null)),
        Err(PlatformError::UnknownCapability(_))
    ));
}
