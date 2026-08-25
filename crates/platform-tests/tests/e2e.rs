use handl_platform::{CapabilityRequest, Kit, KitManifest};
use handl_platform_tests::MockHost;

#[test]
fn simulated_host_loads_kit_and_dispatches() {
    let mut host = MockHost::new();
    let kit = Kit {
        manifest: KitManifest {
            id: "core".into(),
            name: "core".into(),
            plugins: vec!["fmt".into(), "lint".into()],
            target: None,
            selectors: Vec::new(),
        },
        available_plugins: Vec::new(),
    };
    host.load_kit(kit);

    let capabilities = host.plugins.capabilities();
    assert_eq!(capabilities.len(), 2);
    let ids: Vec<_> = capabilities.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"fmt"));
    assert!(ids.contains(&"lint"));

    let response = host
        .run(&CapabilityRequest {
            capability: "fmt".into(),
            payload: serde_json::Value::Null,
        })
        .unwrap();
    assert_eq!(
        response.0,
        serde_json::json!({"kit":"core","plugin":"fmt"})
    );

    let kits = host.kits.all();
    assert_eq!(kits.len(), 1);
    assert_eq!(kits[0].manifest.name, "core");
}
