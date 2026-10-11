//! The `typecheck` capability plugin.
//!
//! ## Payload contract (capability `typecheck`)
//!
//! ```json
//! { "source": "<text>", "prelude": ["str", "map"] }
//! ```
//!
//! - `source` (required, string) — the text to check.
//! - `prelude` (optional, array of strings) — extra names to seed into module
//!   scope before unbound-wire detection, on top of the kit's canonical wire
//!   names. Omitted = the canonical set alone.
//!
//! ## Response
//!
//! - `{ "status": "ok", "type": "<synthesized type>" }`
//! - `{ "status": "type-errors", "message": "<why>" }` — the source checked
//!   and reported type errors. This is a *reported failure*, not a transport
//!   failure: `hnl typecheck` prints it to stdout and exits 0.
//! - `{ "status": "error", "message": "<why>" }` — could not check (lex /
//!   parse / language error). `hnl typecheck` prints it to stderr and exits 2.
//!
//! Only a malformed *request* is a [`PlatformError`]. The split between
//! `type-errors` and `error` mirrors `host-cli`'s existing exit-code contract
//! exactly, so the reroute through this plugin is lossless.

use std::cell::RefCell;
use std::collections::HashSet;

use handl_kit_canonical::Kit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};

/// Capability id served by [`TypecheckPlugin`].
pub const TYPECHECK_CAPABILITY: &str = "typecheck";

thread_local! {
    /// One assembled [`Kit`] per thread.
    ///
    /// `Kit::new()` assembles the canonical language and binds the engines —
    /// the same cost `host-cli` pays once per `hnl typecheck` invocation. A
    /// `Kit` holds `RefCell`s and so is not `Sync`; memoizing per thread keeps
    /// the CLI's "build once, check every file" behaviour without a mutex.
    /// This mirrors the `thread_local!` engine caches in `handl-runtime` and
    /// the `plugin-engine-*` crates.
    static KIT: RefCell<Option<Kit>> = const { RefCell::new(None) };
}

/// Run `f` with the memoized kit, assembling it on first use.
///
/// The canonical language is compiled-in data, so assembly failing means a
/// corrupt build rather than a recoverable condition — panic instead of
/// degrading, matching `plugin-ext-lint::lint_language`. This also keeps the
/// happy path the *only* path, so the plugin has no untestable error arm.
fn with_kit<R>(f: impl FnOnce(&Kit) -> R) -> R {
    KIT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Kit::new().expect("canonical kit must assemble"));
        }
        let kit = slot.as_ref().expect("kit populated above");
        f(kit)
    })
}

/// Adapts the canonical typechecker to the capability contract.
pub struct TypecheckPlugin {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckPlugin {
    /// Create the plugin, declaring the `typecheck` capability.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from(TYPECHECK_CAPABILITY)],
        }
    }
}

impl Default for TypecheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TypecheckPlugin {
    fn id(&self) -> &str {
        "ext.typecheck"
    }

    fn name(&self) -> &str {
        "Typechecker"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if capability.as_str() != TYPECHECK_CAPABILITY {
            return None;
        }
        Some(serde_json::json!({
            "id": TYPECHECK_CAPABILITY,
            "description": "Typecheck canonical handl source (ADR-0088)",
        }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != TYPECHECK_CAPABILITY {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let source = request
            .payload
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PlatformError::InvalidMetadata("typecheck: missing string field `source`".into())
            })?;
        // Absent `prelude` = the canonical wire names alone; an explicit
        // (possibly empty) array is honoured as given.
        let prelude: HashSet<String> = match request.payload.get("prelude") {
            None | Some(serde_json::Value::Null) => HashSet::new(),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            Some(_) => {
                return Err(PlatformError::InvalidMetadata(
                    "typecheck: `prelude` must be an array of strings".into(),
                ));
            }
        };

        let outcome = with_kit(|kit| kit.typecheck_handl_with_prelude(source, &prelude));
        let response = match outcome {
            Ok(typ) => serde_json::json!({ "status": "ok", "type": format!("{typ}") }),
            Err(err) => {
                let message = format!("{err}");
                // `host-cli` treats "typecheck errors: …" as a reported
                // failure (stdout, exit 0) and everything else — lex, parse,
                // language errors — as a hard error (stderr, exit 2).
                let status = if message.contains("typecheck errors:") {
                    "type-errors"
                } else {
                    "error"
                };
                serde_json::json!({ "status": status, "message": message })
            }
        };
        Ok(CapabilityResponse(response))
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
            capability: CapabilityId::from(TYPECHECK_CAPABILITY),
            payload,
        }
    }

    #[test]
    fn default_is_new_and_name_is_reported() {
        let plugin = TypecheckPlugin::default();
        assert_eq!(plugin.name(), "Typechecker");
        assert_eq!(plugin.capabilities().len(), 1);
    }

    #[test]
    fn a_null_prelude_is_treated_as_absent() {
        let plugin = TypecheckPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({ "source": "x = 1", "prelude": null })),
                    ctx,
                )
                .expect("typecheck runs")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    #[test]
    fn type_errors_are_reported_as_a_distinct_status() {
        // `host-cli` distinguishes "the checker ran and found type errors"
        // (stdout, exit 0) from "could not check" (stderr, exit 2). The former
        // is the `type-errors` status; an unbound wire produces it.
        let plugin = TypecheckPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({ "source": "x = y;" })), ctx)
                .expect("a type error is a response, not a PlatformError")
        });
        assert_eq!(
            res.0.get("status").and_then(|v| v.as_str()),
            Some("type-errors"),
            "got {}",
            res.0
        );
        assert!(
            res.0
                .get("message")
                .and_then(|v| v.as_str())
                .is_some_and(|m| m.contains("typecheck errors:")),
            "got {}",
            res.0
        );
    }

    #[test]
    fn declares_the_typecheck_capability() {
        let plugin = TypecheckPlugin::new();
        assert_eq!(plugin.id(), "ext.typecheck");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "typecheck");
        assert!(plugin.metadata(&CapabilityId::from("typecheck")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("lint")).is_none());
    }

    #[test]
    fn a_wrong_capability_is_unknown() {
        let plugin = TypecheckPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &CapabilityRequest {
                    capability: CapabilityId::from("lint"),
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
    fn missing_source_is_invalid_metadata() {
        let plugin = TypecheckPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `source` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_array_prelude_is_invalid_metadata() {
        let plugin = TypecheckPlugin::new();
        let err = with_ctx(|ctx| {
            plugin.run(
                &req(serde_json::json!({ "source": "x = 1", "prelude": "str" })),
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
    fn a_well_typed_module_reports_ok_with_a_type() {
        let plugin = TypecheckPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({ "source": "x = 1" })), ctx)
                .expect("typecheck runs")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("ok"));
        assert!(
            res.0.get("type").and_then(|v| v.as_str()).is_some(),
            "ok must carry the synthesized type, got {}",
            res.0
        );
    }

    #[test]
    fn an_unparseable_source_is_an_error_status_not_a_platform_error() {
        // Distinguishes "the checker ran and the source is bad" from "the
        // capability could not run": the former is a response, the latter a
        // `PlatformError`.
        let plugin = TypecheckPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(&req(serde_json::json!({ "source": "@@@" })), ctx)
                .expect("a parse failure is a response, not a PlatformError")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("error"));
        assert!(
            res.0.get("message").and_then(|v| v.as_str()).is_some(),
            "error must carry a message, got {}",
            res.0
        );
    }

    #[test]
    fn an_explicit_prelude_is_honoured() {
        // The seeded name must be in scope: without it the module would report
        // an unbound wire for `provided_name`.
        let plugin = TypecheckPlugin::new();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({
                        "source": "provided_name",
                        "prelude": ["provided_name"],
                    })),
                    ctx,
                )
                .expect("typecheck runs")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("ok"));
    }
}
