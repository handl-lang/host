//! Public `lib-type` façade for the Handl pure type system.
//!
//! Re-exports the public APIs of `handl-plugin-typecheck-nbe` and
//! `handl-kit-type-schema-json`, plus a convenience `typecheck_schema` entry
//! point.
//!
//! ## ADR-0077 Phase 4: op/handl migration
//!
//! The plugin scaffolding has been migrated to the `op`/`handl` declaration
//! form. The type interface is declared in `handl-packages/type/type.hnl`
//! via `---` doc comments, `cap` declarations, and `op` declarations. The
//! [`TypeLibraryPlugin`] wraps a generic [`HandlLibraryPlugin`] for the
//! interface declaration (id, name, capabilities).
//!
//! This is a "Special" crate per the ADR — the typecheck engine itself
//! (NbE-based type synthesis, JSON Schema interpretation) stays in Rust and
//! is re-exported below. Only the plugin scaffolding was migrated.

pub use handl_kit_type_schema_json;
pub use handl_plugin_typecheck_nbe;

pub use handl_kit_type_schema_json::{
    AliasEntry, AliasTable, SchemaError, SchemaInterpreter, default_tctx, interpret,
};
pub use handl_plugin_typecheck_nbe::*;

/// Re-export the prelude environment and typing context constructors.
pub use handl_plugin_typecheck_nbe::primitives::{prelude_env, prelude_tctx};

/// Convenience `typecheck` using a fresh `MetaContext`, the empty `Env`,
/// and the supplied `TCtx`.
pub fn typecheck(term: &Term, tctx: &TCtx) -> Result<Value, TypeError> {
    let mut ctx = MetaContext::new();
    synth(&mut ctx, &Env::new(), tctx, term)
}

/// Convenience `typecheck` using the prelude `TCtx`.
pub fn typecheck_with_prelude(term: &Term) -> Result<Value, TypeError> {
    typecheck(term, &prelude_tctx())
}

/// Typecheck a JSON Schema by interpreting it into a `handl-plugin-typecheck-nbe` `Term`
/// and synthesising its type using the schema bridge's default primitive
/// typing context.
///
/// The `schema` is interpreted with itself as the root document, so
/// local `$defs` are resolved from it.
pub fn typecheck_schema(
    schema: &serde_json::Value,
) -> Result<handl_plugin_typecheck_nbe::Value, handl_plugin_typecheck_nbe::TypeError> {
    let term = handl_kit_type_schema_json::interpret(schema, schema)
        .map_err(|e| handl_plugin_typecheck_nbe::TypeError::CannotSynthesize(e.to_string()))?;
    let mut ctx = handl_plugin_typecheck_nbe::MetaContext::new();
    handl_plugin_typecheck_nbe::synth_type(
        &mut ctx,
        &handl_plugin_typecheck_nbe::Env::new(),
        &handl_kit_type_schema_json::default_tctx(),
        &term,
    )
}

// ---------------------------------------------------------------------------
// TypeLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// The handl source declaring the type interface — the single source of
/// truth for the type library API surface. Loaded at compile time from the
/// kit-stdlib data dir.
#[cfg(feature = "plugin")]
const TYPE_HNL: &str = include_str!("../../../handl-packages/type/type.hnl");

/// A `Library` plugin that declares the `type` interface via the generic
/// [`HandlLibraryPlugin`] with `cap` declarations.
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The type interface is declared in `type.hnl` via `---` doc comments,
/// `cap` declarations, and `op` declarations; the [`HandlLibraryPlugin`]
/// inner is used for the interface declaration — id, name, and capabilities
/// parsed from `type.hnl`.
///
/// This is a "Special" crate: the typecheck engine itself (NbE-based type
/// synthesis, JSON Schema interpretation) stays in Rust and is re-exported
/// above. The plugin's `contribute` delegates to the inner
/// [`HandlLibraryPlugin`], which registers a `HandlModule` with no callable
/// functions (the runtime type introspection ops are registered by
/// `plugin-lib-utils`'s `TypeModule`).
#[cfg(feature = "plugin")]
pub struct TypeLibraryPlugin {
    inner: handl_plugin_lib_handl::HandlLibraryPlugin,
}

#[cfg(feature = "plugin")]
impl TypeLibraryPlugin {
    /// Create a new `TypeLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: handl_plugin_lib_handl::HandlLibraryPlugin::new_with_caps(
                "type",
                "lib.type",
                "lib-type",
                TYPE_HNL,
                &[],
            ),
        }
    }
}

#[cfg(feature = "plugin")]
impl Default for TypeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "plugin")]
impl handl_platform::Plugin for TypeLibraryPlugin {
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

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`TypeLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_type::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
#[cfg(feature = "plugin")]
pub use TypeLibraryPlugin as LibraryPlugin;

#[cfg(test)]
mod tests;
