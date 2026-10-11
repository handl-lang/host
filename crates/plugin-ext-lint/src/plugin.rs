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
//! { "source": "<text>", "fix": false, "prelude": ["str", "map"],
//!   "language": { …serialized Language… } }
//! ```
//!
//! - `source` (required, string) — the text to lint.
//! - `fix` (optional, bool, default `false`) — when `true`, return the fixed
//!   source instead of the diagnostics.
//! - `prelude` (optional, array of strings) — canonical wire names the
//!   checker should treat as bound. Omitted = the crate's default prelude.
//! - `language` (optional, object, ADR-0124) — a **serialized `Language`**
//!   (`handl-language`'s `Language` derives `Serialize`/`Deserialize`; the
//!   canonical one is ~180 KiB compact JSON). Omitted = the crate builds its
//!   own default language.
//!
//!   This exists because the caller's language and the crate's built-in
//!   default are not guaranteed identical — `host-cli` lints through
//!   `load_canonical_language()` while the default here is `lint_language()`.
//!   Passing the language makes the capability path match the direct call
//!   instead of approximating it.
//!
//! ## Response
//!
//! Discriminated by `status`, matching the `fmt` capability, so no case is
//! inferred from a missing field:
//!
//! - `{ "status": "diagnostics", "diagnostics": [ <Diagnostic>, ... ] }`
//! - `{ "status": "fixed", "output": "<fixed text>" }`
//! - `{ "status": "error", "message": "<why>" }` — could not lint or fix.
//!
//! Only a malformed *request* is a [`PlatformError`] (bad payload, wrong
//! capability, un-deserializable `language`); a parse failure is a successful
//! response carrying an error status.

use handl_language::Language;
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
        let source = request
            .payload
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
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
                ));
            }
        };

        // ADR-0124 primary path: the payload may carry the caller's language.
        // Absent = build our own default (today's behaviour). A malformed
        // language is `InvalidMetadata` rather than a silent fallback, since
        // silently linting with the wrong ruleset is worse than failing.
        let language: Option<Language> = match request.payload.get("language") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| {
                PlatformError::InvalidMetadata(format!("lint: `language` did not deserialize: {e}"))
            })?),
        };

        if fix {
            // No `fix_source_with_language` variant exists, so a caller-supplied
            // `language` cannot be honoured on the fix path: fixing applies the
            // crate's own ruleset. Documented rather than silently ignored.
            match match &prelude {
                Some(p) => crate::fix_source_with_prelude(source, p),
                None => crate::fix_source(source),
            } {
                Ok(output) => Ok(CapabilityResponse(
                    serde_json::json!({ "status": "fixed", "output": output }),
                )),
                Err(e) => Ok(CapabilityResponse(serde_json::json!({
                    "status": "error",
                    "message": format!("{e}"),
                }))),
            }
        } else {
            match match (&language, &prelude) {
                (Some(l), Some(p)) => crate::lint_source_with_prelude_and_language(l, source, p),
                (Some(l), None) => crate::lint_source_with_language(l, source),
                (None, Some(p)) => crate::lint_source_with_prelude(source, p),
                (None, None) => crate::lint_source(source),
            } {
                Ok(diagnostics) => {
                    // `Diagnostic` is plain data (strings/numbers/options), so
                    // serialization cannot fail — panic rather than carry an
                    // untestable fallback arm.
                    let json = serde_json::to_value(&diagnostics)
                        .expect("lint diagnostics must serialize");
                    Ok(CapabilityResponse(serde_json::json!({
                        "status": "diagnostics",
                        "diagnostics": json,
                    })))
                }
                // Like `fmt`, a parse failure is a *response*, not a
                // `PlatformError`, so a caller can distinguish "lint found
                // problems" from "could not lint" without string-matching.
                Err(e) => Ok(CapabilityResponse(serde_json::json!({
                    "status": "error",
                    "message": format!("{e}"),
                }))),
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
        assert_eq!(
            res.0.get("status").and_then(|v| v.as_str()),
            Some("diagnostics")
        );
        assert!(
            res.0
                .get("diagnostics")
                .and_then(|v| v.as_array())
                .is_some(),
            "lint must answer with a `diagnostics` array, got {}",
            res.0
        );
    }

    #[test]
    fn fix_returns_output() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({"source": "x = 1", "fix": true})),
                    ctx,
                )
                .expect("lint fix succeeds")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("fixed"));
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
    fn default_is_new_and_name_is_reported() {
        let plugin = LintPlugin::default();
        assert_eq!(plugin.name(), "Linter");
        assert_eq!(plugin.capabilities().len(), 1);
    }

    #[test]
    fn a_wrong_capability_is_unknown() {
        let plugin = LintPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &CapabilityRequest {
                    capability: CapabilityId::from("fmt"),
                    payload: serde_json::json!({ "source": "x = 1" }),
                },
                ctx,
            )
        })
        .expect_err("a mismatched capability must fail");
        assert!(
            matches!(err, PlatformError::UnknownCapability(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn fix_honours_an_explicit_prelude() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({
                        "source": "x = 1",
                        "fix": true,
                        "prelude": [],
                    })),
                    ctx,
                )
                .expect("fix with an explicit prelude succeeds")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("fixed"));
    }

    #[test]
    fn fix_reports_a_lex_error_as_a_response() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({
                        "source": "x = \"unterminated;",
                        "fix": true,
                    })),
                    ctx,
                )
                .expect("a lint failure is a response, not a PlatformError")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("error"));
        assert!(res.0.get("message").is_some(), "got {}", res.0);
    }

    #[test]
    fn lint_with_both_a_language_and_a_prelude_succeeds() {
        // Exercises the `(Some(language), Some(prelude))` dispatch arm.
        let plugin = LintPlugin::new();
        let lang =
            handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles");
        let serialized = serde_json::to_value(&lang).expect("Language serializes");
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({
                        "source": "x = 1",
                        "language": serialized,
                        "prelude": ["str"],
                    })),
                    ctx,
                )
                .expect("lint succeeds")
        });
        assert!(res.0.get("diagnostics").is_some(), "got {}", res.0);
    }

    #[test]
    fn lint_reports_a_lex_error_as_a_response() {
        let plugin = LintPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({ "source": "x = \"unterminated;" })),
                    ctx,
                )
                .expect("a lint failure is a response, not a PlatformError")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("error"));
        assert!(
            res.0
                .get("message")
                .and_then(|v| v.as_str())
                .is_some_and(|m| !m.is_empty()),
            "got {}",
            res.0
        );
    }

    #[test]
    fn missing_source_is_invalid_metadata() {
        let plugin = LintPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `source` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
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
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_serialized_language_is_accepted() {
        // ADR-0124 primary path: the payload carries the caller's language.
        // This is what makes a capability-path lint match a direct call, since
        // the caller's language need not be this crate's built-in default.
        let plugin = LintPlugin::new();
        let lang =
            handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles");
        let serialized = serde_json::to_value(&lang).expect("Language serializes");
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({ "source": "x = 1", "language": serialized })),
                    ctx,
                )
                .expect("lint with an explicit language succeeds")
        });
        assert!(
            res.0
                .get("diagnostics")
                .and_then(|v| v.as_array())
                .is_some(),
            "expected diagnostics, got {}",
            res.0
        );
    }

    #[test]
    fn a_malformed_language_is_invalid_metadata() {
        // Not a silent canonical fallback: linting with the wrong ruleset is
        // worse than failing loudly.
        let plugin = LintPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &req(serde_json::json!({ "source": "x = 1", "language": 5 })),
                ctx,
            )
        })
        .expect_err("a non-language `language` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }
}
