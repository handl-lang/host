//! Public `lib-type` façade for the Handl pure type system.
//!
//! Re-exports the public APIs of `handl-plugin-typecheck-nbe` and `handl-kit-type-schema-json`,
//! plus a convenience `typecheck_schema` entry point.

pub use handl_plugin_typecheck_nbe;
pub use handl_kit_type_schema_json;

pub use handl_plugin_typecheck_nbe::*;
pub use handl_kit_type_schema_json::{
    AliasEntry, AliasTable, SchemaError, SchemaInterpreter, default_tctx, interpret,
};

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

#[cfg(test)]
mod tests;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "plugin")]
pub use plugin::LibraryPlugin;
