//! `handl-plugin-ext-fmt` is a thin re-export of the platform `Format`
//! operation and the data-driven [`FormatEngine`].

pub use handl_platform::{BasicFormatter, Format, FormatBehavior, FormatRequest, RuleAcceptor};
pub use handl_plugin_engine_fmt::FormatEngine;

use handl_language::{Ast, AstRef, LangError, Language};

/// Format the subtree rooted at `root` in `ast` using `lang` data.
///
/// This is the primary formatting entry point for callers that have a full
/// [`Ast`] arena (as opposed to the single-node [`FormatRequest`] used by the
/// `BasicFormatter` rule-acceptor plugin).
///
/// # Errors
///
/// Returns [`LangError`] if the formatter encounters a malformed AST.
pub fn format_ast(lang: &Language, ast: &Ast, root: AstRef) -> Result<String, LangError> {
    FormatEngine::new().format(lang, ast, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, Format, FormatRequest, KitRegistry,
        PlatformError, Plugin, PluginContext, PluginRegistry,
    };

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicFormatter::new();
        let meta = plugin.metadata(&CapabilityId::from("fmt")).unwrap();
        assert_eq!(meta["command"], "fmt");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_formatted_output_when_rule_matches() {
        struct Echo;

        impl Behavior<Format> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }

            fn apply(&self, op: &mut FormatRequest) -> Result<(), PlatformError> {
                op.output = op.ast.payload.to_string();
                Ok(())
            }
        }

        impl FormatBehavior for Echo {}

        let mut formatter = BasicFormatter::new();
        formatter.accept(Box::new(Echo)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = formatter
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::json!({"kind": "echo", "value": 42}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(
            resp.0.get("formatted").is_some(),
            "expected formatted output in response"
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicFormatter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
