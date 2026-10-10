//! The `fmt` capability plugin — the capability-contract adapter over the
//! free functions in this crate.
//!
//! `host-cli` currently calls [`crate::format_source`] / [`crate::check_source`]
//! directly. This adapter exposes the same behaviour through the
//! `plugin-ext-*` capability seam so a host (`host-cli`, `host-lsp`, a third
//! party) can dispatch `{command: "fmt", args: [...]}` uniformly. The free
//! functions remain the implementation.
//!
//! ## Payload contract (capability `fmt`)
//!
//! ```json
//! { "source": "<text>", "check": false }
//! ```
//!
//! - `source` (required, string) — the text to format.
//! - `check` (optional, bool, default `false`) — when `true`, report whether
//!   `source` is already formatted instead of returning the formatted text.
//!
//! ## Response
//!
//! - `check: true`  → `{ "ok": <bool> }` — `ok` is `true` when the input is
//!   already canonically formatted (`check_source`).
//! - `check: false` → `{ "output": "<formatted text>" }`.
//!
//! A malformed payload is reported as [`PlatformError::InvalidMetadata`]; a
//! formatting failure is reported as [`PlatformError::Plugin`].

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};

/// Capability id served by [`FmtPlugin`].
pub const FMT_CAPABILITY: &str = "fmt";

/// Adapts this crate's formatter to the capability contract.
pub struct FmtPlugin {
    capabilities: Vec<CapabilityId>,
}

impl FmtPlugin {
    /// Create the plugin, declaring the `fmt` capability.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from(FMT_CAPABILITY)],
        }
    }
}

impl Default for FmtPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FmtPlugin {
    fn id(&self) -> &str {
        "ext.fmt"
    }

    fn name(&self) -> &str {
        "Formatter"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if capability.as_str() != FMT_CAPABILITY {
            return None;
        }
        Some(serde_json::json!({
            "id": FMT_CAPABILITY,
            "description": "Format canonical handl source (ADR-0085)",
        }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != FMT_CAPABILITY {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let source = request
            .payload
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::InvalidMetadata("fmt: missing string field `source`".into()))?;
        let check = request
            .payload
            .get("check")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if check {
            let ok = crate::check_source(source)
                .map_err(|e| PlatformError::Plugin(format!("fmt check failed: {e}")))?;
            Ok(CapabilityResponse(serde_json::json!({ "ok": ok })))
        } else {
            let output = crate::format_source(source)
                .map_err(|e| PlatformError::Plugin(format!("fmt failed: {e}")))?;
            Ok(CapabilityResponse(serde_json::json!({ "output": output })))
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
            capability: CapabilityId::from(FMT_CAPABILITY),
            payload,
        }
    }

    #[test]
    fn declares_the_fmt_capability() {
        let plugin = FmtPlugin::new();
        assert_eq!(plugin.id(), "ext.fmt");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "fmt");
        assert!(plugin.metadata(&CapabilityId::from("fmt")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("nope")).is_none());
    }

    #[test]
    fn check_reports_whether_source_is_formatted() {
        let plugin = FmtPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({"source": "x = 1", "check": true})), ctx)
                .expect("fmt check succeeds")
        });
        assert!(
            res.0.get("ok").and_then(|v| v.as_bool()).is_some(),
            "check=true must answer with a bool `ok`, got {}",
            res.0
        );
    }

    #[test]
    fn format_returns_output() {
        let plugin = FmtPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({"source": "x = 1"})), ctx)
                .expect("fmt succeeds")
        });
        assert!(
            res.0.get("output").and_then(|v| v.as_str()).is_some(),
            "check=false must answer with `output`, got {}",
            res.0
        );
    }

    #[test]
    fn missing_source_is_invalid_metadata() {
        let plugin = FmtPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({"check": true})), ctx))
            .expect_err("a payload without `source` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "expected InvalidMetadata, got {err:?}"
        );
    }

    #[test]
    fn wrong_capability_is_unknown() {
        let plugin = FmtPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &CapabilityRequest {
                    capability: CapabilityId::from("lint"),
                    payload: serde_json::json!({"source": "x"}),
                },
                ctx,
            )
        })
        .expect_err("the fmt plugin must reject the lint capability");
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
