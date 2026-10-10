//! The `lint` capability plugin — the capability-contract adapter over the
//! free functions in this crate.
//!
//! `host-cli` currently calls [`crate::lint_source_with_prelude`] /
//! [`crate::fix_source_with_prelude`] directly. This adapter exposes the same
//! behaviour through the `plugin-ext-*` capability seam so a host can
//! dispatch `{command: "lint", args: [...]}` uniformly. The free functions
//! remain the implementation.
//!
//! ## Payload contract (capability `lint`)
//!
//! ```json
//! { "source": "<text>", "fix": false, "prelude": ["str", "map"] }
//! ```
//!
//! - `source` (required, string) — the text to lint.
//! - `fix` (optional, bool, default `false`) — when `true`, return the fixed
//!   source instead of the diagnostics.
//! - `prelude` (optional, array of strings) — canonical wire names the
//!   checker should treat as bound (see `canonical_wire_names`). Omitted =
//!   the crate's default prelude.
//!
//! ## Response
//!
//! - `fix: false` → `{ "diagnostics": [ <Diagnostic>, ... ] }`
//! - `fix: true`  → `{ "output": "<fixed text>" }`
//!
//! A malformed payload is reported as [`PlatformError::InvalidMetadata`]; a
//! lint failure is reported as [`PlatformError::Plugin`].

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use std::collections::HashSet;

/// Capability id served by [`LintPlugin`].
pub const LINT_CAPABILITY: &str = "lint";

/// Adapts this crate's linter to the capability contract.
pub struct LintPlugin {
    capabilities: Vec<CapabilityId>,
}

impl LintPlugin {
    /// Create the plugin, declaring the `lint` capability.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from(LINT_CAPABILITY)],
        }
    }
}

impl Default for LintPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for LintPlugin {
    fn id(&self) -> &str {
        "ext.lint"
    }

    fn name(&self) -> &str {
        "Linter"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if capability.as_str() != LINT_CAPABILITY {
            return None;
        }
        Some(serde_json::json!({
            "id": LINT_CAPABILITY,
            "description": "Lint canonical handl source (ADR-0097)",
        }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != LINT_CAPABILITY {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let source = request.payload.get("source").and_then(|v| v.as_str()).ok_or_else(|| {
            PlatformError::InvalidMetadata("lint: missing string field `source`".into())
        })?;
        let fix = request
            .payload
            .get("fix")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        // Absent `prelude` = the crate default; an explicit (possibly empty)
        // array is honoured as given.
        let prelude: Option<HashSet<String>> = match request.payload.get("prelude") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Array(items)) => Some(
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
            ),
            Some(_) => {
                return Err(PlatformError::InvalidMetadata(
                    "lint: `prelude` must be an array of strings".into(),
                ))
            }
        };

        if fix {
            let output = match &prelude {
                Some(p) => crate::fix_source_with_prelude(source, p),
                None => crate::fix_source(source),
            }
            .map_err(|e| PlatformError::Plugin(format!("lint fix failed: {e}")))?;
            Ok(CapabilityResponse(serde_json::json!({ "output": output })))
        } else {
            let diagnostics = match &prelude {
                Some(p) => crate::lint_source_with_prelude(source, p),
                None => crate::lint_source(source),
            }
            .map_err(|e| PlatformError::Plugin(format!("lint failed: {e}")))?;
            let json = serde_json::to_value(&diagnostics).map_err(|e| {
                PlatformError::Plugin(format!("lint: diagnostics not serializable: {e}"))
            })?;
            Ok(CapabilityResponse(serde_json::json!({ "diagnostics": json })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{KitRegistry, PluginRegistry};

    fn with_ctx<R>(f: impl FnOnce(&mut PluginContext) -> R) -> R {
        let mut plugins = PluginRegistry::new();
        let mut kits = KitRegistry::new();
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        f(&mut ctx)
    }

    fn req(payload: serde_json::Value) -> CapabilityRequest {
        CapabilityRequest {
            capability: CapabilityId::from(LINT_CAPABILITY),
            payload,
        }
    }

    #[test]
    fn declares_the_lint_capability() {
        let plugin = LintPlugin::new();
        assert_eq!(plugin.id(), "ext.lint");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "lint");
        assert!(plugin.metadata(&CapabilityId::from("lint")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("fmt")).is_none());
    }

    #[test]
    fn lint_returns_a_diagnostics_array() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({"source": "x = 1"})), ctx)
                .expect("lint succeeds")
        });
        assert!(
            res.0.get("diagnostics").and_then(|v| v.as_array()).is_some(),
            "lint must answer with a `diagnostics` array, got {}",
            res.0
        );
    }

    #[test]
    fn fix_returns_output() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({"source": "x = 1", "fix": true})), ctx)
                .expect("lint fix succeeds")
        });
        assert!(
            res.0.get("output").and_then(|v| v.as_str()).is_some(),
            "fix=true must answer with `output`, got {}",
            res.0
        );
    }

    #[test]
    fn an_explicit_empty_prelude_is_honoured() {
        // Distinguishes "omitted" (crate default) from "explicitly empty",
        // which is the whole reason `prelude` is `Option<HashSet<_>>`.
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({"source": "x = 1", "prelude": []})),
                    ctx,
                )
                .expect("lint with an explicit empty prelude succeeds")
        });
        assert!(res.0.get("diagnostics").is_some());
    }

    #[test]
    fn missing_source_is_invalid_metadata() {
        let plugin = LintPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `source` must fail");
        assert!(matches!(err, PlatformError::InvalidMetadata(_)), "got {err:?}");
    }

    #[test]
    fn a_non_array_prelude_is_invalid_metadata() {
        let plugin = LintPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &req(serde_json::json!({"source": "x = 1", "prelude": "str"})),
                ctx,
            )
        })
        .expect_err("a string `prelude` must fail");
        assert!(matches!(err, PlatformError::InvalidMetadata(_)), "got {err:?}");
    }
}
