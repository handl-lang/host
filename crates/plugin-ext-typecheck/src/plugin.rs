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

use std::collections::HashSet;
use std::sync::Arc;

use handl_language::{LangError, Value};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};

/// Capability id served by [`TypecheckPlugin`].
pub const TYPECHECK_CAPABILITY: &str = "typecheck";

/// The typechecker this plugin adapts: `(source, extra_prelude)` → the
/// synthesized [`Value`] type or a [`LangError`].
///
/// Injected at construction (ADR-0128 §"The 0.8 seam"): `Plugin` requires
/// `Send + Sync` and a `Kit` is `!Sync`, so the host owns the kit — in a
/// thread-local cache, memoized as before — and injects a capture-less
/// closure that reads it. The plugin is then a pure adapter: it carries no
/// `handl-kit-canonical` type, and the crate keeps no normal dependency on
/// the canonical kit.
pub type TypecheckFn =
    dyn Fn(&str, &HashSet<String>) -> Result<Value, LangError> + Send + Sync;

/// Adapts an injected typechecker to the capability contract.
pub struct TypecheckPlugin {
    capabilities: Vec<CapabilityId>,
    check: Arc<TypecheckFn>,
}

impl TypecheckPlugin {
    /// Create the plugin, declaring the `typecheck` capability.
    ///
    /// `check` performs the typecheck. The host supplies it — e.g.
    /// `handl_kit_prelude::Kit::typecheck_handl_with_prelude` bound to a
    /// thread-cached canonical kit.
    pub fn new(check: Arc<TypecheckFn>) -> Self {
        Self {
            capabilities: vec![CapabilityId::from(TYPECHECK_CAPABILITY)],
            check,
        }
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

        let outcome = (self.check)(source, &prelude);
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
    use std::cell::RefCell;

    use handl_kit_prelude::Kit;
    use handl_platform::{KitRegistry, PluginRegistry};

    thread_local! {
        /// The canonical kit, memoized per test binary thread — the same
        /// host-side cache shape the production hosts keep (ADR-0128 §0.8).
        static KIT: RefCell<Option<Kit>> = const { RefCell::new(None) };
    }

    /// The canonical `typecheck` capability, bound to the thread-cached kit.
    fn canonical_check() -> Arc<TypecheckFn> {
        Arc::new(|src, prelude| {
            KIT.with(|cell| {
                let mut slot = cell.borrow_mut();
                if slot.is_none() {
                    *slot = Some(Kit::new().expect("canonical kit must assemble"));
                }
                slot.as_ref()
                    .expect("kit populated above")
                    .typecheck_handl_with_prelude(src, prelude)
            })
        })
    }

    /// A plugin wired with the canonical checker.
    fn plugin() -> TypecheckPlugin {
        TypecheckPlugin::new(canonical_check())
    }

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
        let plugin = plugin();
        assert_eq!(plugin.name(), "Typechecker");
        assert_eq!(plugin.capabilities().len(), 1);
    }

    #[test]
    fn a_null_prelude_is_treated_as_absent() {
        let plugin = plugin();
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
        let plugin = plugin();
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
        let plugin = plugin();
        assert_eq!(plugin.id(), "ext.typecheck");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "typecheck");
        assert!(plugin.metadata(&CapabilityId::from("typecheck")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("lint")).is_none());
    }

    #[test]
    fn a_wrong_capability_is_unknown() {
        let plugin = plugin();
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
        let plugin = plugin();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `source` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_array_prelude_is_invalid_metadata() {
        let plugin = plugin();
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
        let plugin = plugin();
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
        let plugin = plugin();
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
        let plugin = plugin();
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
