//! # handl-plugin-lib-tag
//!
//! The **tagged-values** capability as a composable `plugin-lib-*` leaf crate
//! (ADR-0055 — Tagged Values).
//!
//! Tagged values are the host primitive that lets the typechecker, units,
//! capabilities, and provenance share one value space without collision
//! (ADR-0032 "types are values"). A tagged value is a JSON object
//! `{"__tag__": <name>, ...payload}` minted by the host. Tagging is a **host
//! capability**, not a user-constructible convention: the `__tag__` key is
//! reserved (the eval engines reject it in user object literals), so the
//! only way a value acquires a `__tag__` key is through this plugin's
//! `tag.tag` host function. User code cannot forge a tagged value.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The tag interface is declared in
//! `handl-packages/tag/tag.hnl` via `op` declarations, and the Rust
//! tag/match/inspect logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`TagLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! This crate exposes the `tag` module with four callables — inspection
//! and dispatch only; the `__tag__` mint producers (`tag.tag`,
//! `tag.construct`) were deleted with the `__tag__`/`{tag}` bridge
//! (ADR-0152 §Migration):
//!
//! | Callable | Role |
//! |---|---|
//! | `tag.untag(v)` | return the payload of a tagged value (inspection) |
//! | `tag.tag_of(v)` | return the tag name of a tagged value (inspection) |
//! | `tag.is_tag(v)` | test whether `v` is a tagged value (inspection) |
//! | `tag.match_type(v, cases)` | **dispatch** on the type tag of `v` (ADR-0054) |
//!
//! `tag.match_type` is the user-facing surface of the ADR-0054 judgment
//! op `match_type`, backed by a [`TypeRegistry`] with registry id
//! `"handl.canonical"`.
//!
//! Tagged values are still produced internally (law/rewrite/cell
//! records); the inspection ops are available to any code that holds
//! one — matching the capability discipline of ADR-0055 §1.
//! Tag dispatch on `__tag__` identity is handled by the native `match`
//! construct via tag patterns (`ok(v) => ...`), not a dedicated op
//! (ADR-0077).
//!
//! The [`TypeRegistry`] (in [`registry`]) is the type system machinery, not a
//! callable — it is kept in Rust as pure data.
//!
//! The [`TagModule`] re-export is preserved for backward compatibility with
//! downstream crates (e.g. [`kit-canonical`]) that register `TagModule`
//! directly.

#![warn(missing_docs)]

use std::sync::Arc;

use handl_language::{Value, as_tag, is_tag_value};
use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

/// The host type registry (ADR-0054).
pub mod registry;
pub use registry::TypeRegistry;

#[cfg(test)]
mod tests;

/// The handl source declaring the tag `op` interface — the single source
/// of truth for the tag API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const TAG_HNL: &str = handl_lib_tag::TAG_HNL;

/// The op names declared in `tag.hnl` — inspection ops only. The
/// `__tag__` mint producers (`tag.tag`/`tag.construct`) were deleted
/// with the `__tag__`/`{tag}` bridge (ADR-0152 §Migration): user code
/// no longer mints tagged values; host-internal `__tag__` records
/// (Cell/Arrow/Handler/law) remain inspectable.
const TAG_OPS: &[&str] = &[
    "tag.untag",
    "tag.tag_of",
    "tag.is_tag",
    "tag.match_type",
];

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// Extract the payload of a tagged value: a clone of `v` with the `__tag__`
/// key removed. For a non-tagged value, returns `v` unchanged.
fn payload_of(v: &Value) -> Value {
    if let Some(obj) = v.as_object() {
        if obj.contains_key(handl_language::TAG_VALUE_KEY) {
            let mut map = obj.clone();
            map.remove(handl_language::TAG_VALUE_KEY);
            return Value::Object(map);
        }
    }
    v.clone()
}

// ---------------------------------------------------------------------------
// Inspection ops — untag / tag_of / is_tag
// ---------------------------------------------------------------------------

/// `tag.untag(v)` — return the payload of a tagged value (ADR-0055 §1).
///
/// Returns `v` with the `__tag__` key removed. Returns `null` if `v` is not a
/// tagged value.
///
/// This is the eval-time `Function` impl registered under the op name
/// `"tag.untag"` in the [`OpRegistry`].
#[derive(Clone, Copy)]
pub struct Untag;

impl Function for Untag {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.untag")?;
        if !is_tag_value(v) {
            return Ok(Value::Null);
        }
        Ok(payload_of(v))
    }
}

/// `tag.tag_of(v)` — return the tag name of a tagged value (ADR-0055 §1).
///
/// Returns the tag name string, or `null` if `v` is not a tagged value.
///
/// This is the eval-time `Function` impl registered under the op name
/// `"tag.tag_of"` in the [`OpRegistry`].
#[derive(Clone, Copy)]
pub struct TagOf;

impl Function for TagOf {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.tag_of")?;
        Ok(match as_tag(v) {
            Some(name) => Value::from(name),
            None => Value::Null,
        })
    }
}

/// `tag.is_tag(v)` — test whether `v` is a tagged value (ADR-0055 §1).
///
/// Returns `true` iff `v` is a tagged value minted by the host `tag`
/// capability (i.e. an object carrying a string `__tag__` key).
///
/// This is the eval-time `Function` impl registered under the op name
/// `"tag.is_tag"` in the [`OpRegistry`].
#[derive(Clone, Copy)]
pub struct IsTag;

impl Function for IsTag {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.is_tag")?;
        Ok(Value::from(is_tag_value(v)))
    }
}

/// Require exactly one argument, with a function-name prefix for errors.
fn require_one_arg<'a>(args: &'a [Value], fn_name: &str) -> Result<&'a Value, EvalError> {
    args.first().ok_or_else(|| {
        EvalError::Other(format!(
            "{fn_name}: expected 1 argument, got {}",
            args.len()
        ))
    })
}

// ---------------------------------------------------------------------------
// tag.construct / tag.match_type — host type registry (ADR-0054)
// ---------------------------------------------------------------------------

/// The registry id of the canonical handl typechecker, used by the
/// `tag.match_type` host function (and formerly `tag.construct`, the
/// deleted `__tag__` type-value producer).
pub const CANONICAL_REGISTRY_ID: &str = "handl.canonical";

/// `tag.match_type(v, cases)` — dispatch on the type tag of `v` (ADR-0054
/// §"match_type").
///
/// `cases` is an object mapping type tag names to case values. If `v` is a
/// type value minted by the canonical registry whose tag has an explicit
/// case, that case is returned. Otherwise returns `null` — there is no
/// default-case fallback (the caller handles non-matches, per ADR-0054).
/// This is `match_tag` on type tags scoped to the canonical registry.
///
/// This is the eval-time `Function` impl registered under the op name
/// `"tag.match_type"` in the [`OpRegistry`].
#[derive(Clone, Copy)]
pub struct MatchType;

impl Function for MatchType {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "tag.match_type: expected 2 arguments (value, cases), got {}",
                args.len()
            )));
        }
        let registry = TypeRegistry::new(CANONICAL_REGISTRY_ID);
        Ok(registry
            .match_type(&args[0], &args[1])
            .unwrap_or(Value::Null))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for tag, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"tag.untag"` → [`Untag`] (no runtime checker; axioms trusted)
/// - `"tag.tag_of"` → [`TagOf`] (no runtime checker; axioms trusted)
/// - `"tag.is_tag"` → [`IsTag`] (no runtime checker; axioms trusted)
/// - `"tag.match_type"` → [`MatchType`] (no runtime checker; axioms trusted)
///
/// The `tag.tag`/`tag.construct` producers are deleted — the `__tag__`
/// mint capability retired with the tag bridge (ADR-0152 §Migration).
pub fn tag_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("tag.untag", Arc::new(Untag));
    registry.register("tag.tag_of", Arc::new(TagOf));
    registry.register("tag.is_tag", Arc::new(IsTag));
    registry.register("tag.match_type", Arc::new(MatchType));
    registry
}

// ---------------------------------------------------------------------------
// TagModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

static UNTAG: Untag = Untag;
static TAG_OF: TagOf = TagOf;
static IS_TAG: IsTag = IsTag;
static MATCH_TYPE: MatchType = MatchType;

/// The `tag` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`kit-canonical`]) that register `TagModule` directly. The
/// module exposes `untag`, `tag_of`, `is_tag`, and `match_type`
/// functions that dispatch to the [`OpRegistry`]. The `tag`/`construct`
/// mint producers are deleted — `__tag__` is host-internal data now.
#[derive(Clone, Copy)]
pub struct TagModule;

impl Module for TagModule {
    fn name(&self) -> &str {
        "tag"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "untag" => Some(&UNTAG),
            "tag_of" => Some(&TAG_OF),
            "is_tag" => Some(&IS_TAG),
            "match_type" => Some(&MATCH_TYPE),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("untag", &UNTAG as &dyn Function),
            ("tag_of", &TAG_OF as &dyn Function),
            ("is_tag", &IS_TAG as &dyn Function),
            ("match_type", &MATCH_TYPE as &dyn Function),
        ]
    }
}

/// Re-export the reserved-key predicate for hosts/engines that want to
/// enforce the `__tag__` reservation at construction sites beyond the eval
/// engines (e.g. a future typecheck host).
pub use handl_language::is_reserved_value_key;

// ---------------------------------------------------------------------------
// TagLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `tag` tagged-values capability
/// (ADR-0055) via the generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The tag interface is declared in `tag.hnl` via `op`/`handl`
/// declarations; the Rust tag/match/inspect handls are registered in the op
/// registry. The generic loader handles module registration.
///
/// Registering this plugin offers the `tag.tag` minting capability (and the
/// `tag.untag` / `tag.tag_of` / `tag.is_tag` inspection ops, plus
/// `tag.construct` / `tag.match_type`) to handl programs. A host
/// that does not register this plugin does not offer tagging — user code
/// cannot mint tagged values without it, which is the capability boundary
/// ADR-0055 §1 establishes.
pub struct TagLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl TagLibraryPlugin {
    /// Create a new `TagLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(tag_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("tag", "lib.tag", "lib-tag", TAG_HNL, &[])
                .with_ops(TAG_OPS, registry),
        }
    }
}

impl Default for TagLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for TagLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for tag with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`TagLibraryPlugin`].
pub fn tag_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(tag_op_registry());
    HandlLibraryPlugin::new_with_caps("tag", "lib.tag", "lib-tag", TAG_HNL, &[])
        .with_ops(TAG_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`TagLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_tag::LibraryPlugin`.
pub use TagLibraryPlugin as LibraryPlugin;
