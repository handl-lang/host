use handl_platform::{CapabilityId, CapabilityRequest, DefaultHost, Host};
use handl_ext_compile::CompilePlugin;
use handl_ext_fmt::FmtPlugin;
use handl_ext_lint::LintPlugin;

#[test]
fn mock_host_lists_builtin_capabilities() {
    let mut host = DefaultHost::new();
    host.register_plugin(FmtPlugin::new());
    host.register_plugin(LintPlugin::new());
    host.register_plugin(CompilePlugin::new());

    let caps = host.plugins().capabilities();
    assert_eq!(caps.len(), 3);
    let ids: Vec<_> = caps.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"fmt"));
    assert!(ids.contains(&"lint"));
    assert!(ids.contains(&"compile"));
}

#[test]
fn mock_host_runs_registered_capability() {
    let mut host = DefaultHost::new();
    host.register_plugin(FmtPlugin::new());

    let response = host
        .run(&CapabilityRequest {
            capability: CapabilityId::from("fmt"),
            payload: serde_json::Value::Null,
        })
        .unwrap();
    assert_eq!(response.0, serde_json::json!({"fmtd": true}));
}
