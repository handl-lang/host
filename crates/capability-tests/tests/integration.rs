use handl_platform::{CapabilityId, CapabilityRequest, ExtensionRegistry, KitRegistry, PluginContext, PluginRegistry};
use handl_capability_build::BuildPlugin;
use handl_capability_compile::CompilePlugin;
use handl_capability_eval::EvalPlugin;
use handl_capability_fmt::FmtPlugin;
use handl_capability_lint::LintPlugin;
use handl_capability_typecheck::TypecheckPlugin;

fn test_ctx<'a>(
    plugins: &'a mut PluginRegistry,
    kits: &'a mut KitRegistry,
    extensions: &'a mut ExtensionRegistry,
) -> PluginContext<'a> {
    PluginContext {
        plugins,
        kits,
        extensions,
    }
}

#[test]
fn all_non_management_capabilities_run() {
    let (mut plugins, mut kits, mut extensions) = (
        PluginRegistry::new(),
        KitRegistry::new(),
        ExtensionRegistry::new(),
    );
    let mut ctx = test_ctx(&mut plugins, &mut kits, &mut extensions);

    let cases: Vec<(Box<dyn handl_platform::Plugin>, &str, serde_json::Value)> = vec![
        (Box::new(FmtPlugin::new()), "fmt", serde_json::json!({"fmtd": true})),
        (Box::new(LintPlugin::new()), "lint", serde_json::json!({"linted": true})),
        (Box::new(TypecheckPlugin::new()), "typecheck", serde_json::json!({"typechecked": true})),
        (Box::new(EvalPlugin::new()), "eval", serde_json::json!({"evaled": true})),
        (Box::new(CompilePlugin::new()), "compile", serde_json::json!({"compiled": true})),
        (Box::new(BuildPlugin::new()), "build", serde_json::json!({"built": true})),
    ];

    for (plugin, id, expected) in cases {
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: CapabilityId::from(id),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, expected);
    }
}
