use handl_platform::{CapabilityRequest, CapabilityResponse, ExtensionRegistry, InProcessPlugin, KitRegistry, PluginContext, PluginRegistry};
use handl_platform_tests::MockPlugin;

fn dummy_ctx<'a>(
    plugins: &'a mut PluginRegistry,
    kits: &'a mut KitRegistry,
    extensions: &'a mut ExtensionRegistry,
) -> PluginContext<'a> {
    PluginContext { plugins, kits, extensions }
}

#[test]
fn full_registry_flow() {
    let mut registry = PluginRegistry::new();

    registry.register(MockPlugin::new("echo", &["echo"]).with_output(
        serde_json::json!({"status":"ok"}),
    ));

    let closure = |req: &CapabilityRequest, _ctx: &mut PluginContext| {
        Ok(CapabilityResponse(serde_json::json!({
            "cap": req.capability.as_str(),
            "payload": req.payload,
        })))
    };
    registry.register(InProcessPlugin::new(
        "log",
        &["log".into()],
        serde_json::json!({"command":"log"}),
        closure,
    ));

    let caps = registry.capabilities();
    assert_eq!(caps.len(), 2);

    let echo_cap = caps.iter().find(|(id, _)| id.as_str() == "echo");
    assert!(echo_cap.is_some());

    let mut kits = KitRegistry::new();
    let mut extensions = ExtensionRegistry::new();
    let mut ctx = dummy_ctx(&mut registry, &mut kits, &mut extensions);
    let plugin = ctx.plugins.resolve(&"echo".into()).unwrap();
    let response = plugin
        .run(&CapabilityRequest {
            capability: "echo".into(),
            payload: serde_json::json!({"msg":"hello"}),
        }, &mut ctx)
        .unwrap();
    assert_eq!(response.0, serde_json::json!({"status":"ok"}));

    let ctx2 = dummy_ctx(&mut registry, &mut kits, &mut extensions);
    assert!(ctx2.plugins.resolve(&"missing".into()).is_none());
}
