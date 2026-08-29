//! Generic typecheck extension point.
//!
//! `handl-plugin-ext-typecheck` is now a thin re-export of the platform's generic
//! `Typecheck` operation and `BasicTypechecker` `RuleAcceptor`.  The concrete
//! canonical type-checking logic lives in `handl-plugin-typecheck-core`.

pub use handl_platform::{BasicTypechecker, Typecheck, TypecheckBehavior, TypecheckRequest};

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry};

    #[test]
    fn reexported_basic_typechecker_can_run() {
        let plugin = BasicTypechecker::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn reexported_typecheck_behavior_marker_exists() {
        let _ = Typecheck;
    }
}
