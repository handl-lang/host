//! `handl-plugin-ext-lint` is a thin re-export of the platform `Lint` operation.

pub use handl_platform::{BasicLinter, Lint, LintBehavior, LintRequest, RuleAcceptor};

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, KitRegistry, Lint, LintRequest, PlatformError,
        Plugin, PluginContext, PluginRegistry,
    };

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicLinter::new();
        let meta = plugin.metadata(&CapabilityId::from("lint")).unwrap();
        assert_eq!(meta["command"], "lint");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_diagnostics_when_rule_matches() {
        struct Warn;

        impl Behavior<Lint> for Warn {
            fn kind(&self) -> &str {
                "unused"
            }

            fn apply(&self, op: &mut LintRequest) -> Result<(), PlatformError> {
                op.diagnostics.push("unused variable".to_string());
                Ok(())
            }
        }

        impl LintBehavior for Warn {}

        let mut linter = BasicLinter::new();
        linter.accept(Box::new(Warn)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = linter
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::json!({"kind": "unused"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"diagnostics": ["unused variable"]})
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicLinter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
