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
//! A single `status` discriminates the outcome, so no case is inferred from
//! the *absence* of a field:
//!
//! - `{ "status": "canonical" }` — `check: true`, input already canonical.
//! - `{ "status": "not-canonical" }` — `check: true`, input differs.
//! - `{ "status": "formatted", "output": "<text>" }` — `check: false`.
//! - `{ "status": "error", "diagnostic": { … } }` — could not parse/format.
//!   The structured diagnostic is returned rather than an error string so a
//!   caller can render exactly what `format_error_to_diagnostic` would have.
//!
//! Only a malformed *request* is a [`PlatformError`] (bad payload, wrong
//! capability); a formatting failure is a successful response carrying a
//! diagnostic.

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
            match crate::check_source(source) {
                Ok(true) => Ok(CapabilityResponse(serde_json::json!({ "status": "canonical" }))),
                Ok(false) => {
                    Ok(CapabilityResponse(serde_json::json!({ "status": "not-canonical" })))
                }
                // A parse failure is reported *in the response*, not as a
                // `PlatformError`: the caller needs the structured diagnostic
                // (span included) to render the same message it would have
                // produced calling the crate directly. Flattening it into an
                // error string would be a real regression in output quality.
                Err(err) => {
                    let diag = crate::format_error_to_diagnostic(&err, source);
                    Ok(CapabilityResponse(serde_json::json!({
                        "status": "error",
                        "diagnostic": diag,
                    })))
                }
            }
        } else {
            match crate::format_source(source) {
                Ok(output) => Ok(CapabilityResponse(
                    serde_json::json!({ "status": "formatted", "output": output }),
                )),
                Err(err) => {
                    let diag = crate::format_error_to_diagnostic(&err, source);
                    Ok(CapabilityResponse(serde_json::json!({
                        "status": "error",
                        "diagnostic": diag,
                    })))
                }
            }
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
        let status = res.0.get("status").and_then(|v| v.as_str());
        assert!(
            matches!(status, Some("canonical" | "not-canonical")),
            "check must answer canonical/not-canonical, got {}",
            res.0
        );
    }

    #[test]
    fn a_parse_error_carries_a_structured_diagnostic() {
        // Candidates malformed in different ways. The assertion is that *at
        // least one* produces the error shape, so the test does not depend on
        // exactly which inputs today's parser rejects — writing it against a
        // single guessed input failed twice (the formatter happily formatted
        // both `"x = "` and `"x = (1"`).
        let plugin = FmtPlugin::new();
        let candidates = ["x = (1", ")", "\"unterminated", "@@@", "~[", "x = = 1"];
        let mut saw_error = false;
        for src in candidates {
            let res = with_ctx(|ctx| {
                plugin
                    .run(&req(serde_json::json!({ "source": src })), ctx)
                    .expect("a parse failure is a response, not a PlatformError")
            });
            if res.0.get("status").and_then(|v| v.as_str()) != Some("error") {
                continue;
            }
            let diag = res.0.get("diagnostic").expect("diagnostic present");
            // The span must survive the JSON round-trip, otherwise the reroute
            // would regress error rendering versus the direct call.
            let parsed: Result<handl_plugin_ext_diag::Diagnostic, _> =
                serde_json::from_value(diag.clone());
            assert!(parsed.is_ok(), "diagnostic must deserialize: {diag}");
            saw_error = true;
        }
        assert!(
            saw_error,
            "no candidate produced a parse error, so the error path is untested"
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
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("formatted"));
        assert!(
            res.0.get("output").and_then(|v| v.as_str()).is_some(),
            "formatted must carry `output`, got {}",
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
