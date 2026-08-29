//! Public `lib-type` façade for the Handl pure type system.
//!
//! Re-exports the public APIs of `handl-kit-type` and `handl-plugin-vm-type-schema-json`,
//! plus a convenience `typecheck_schema` entry point.

pub use handl_kit_type;
pub use handl_plugin_vm_type_schema_json;

pub use handl_kit_type::*;
pub use handl_plugin_vm_type_schema_json::{
    AliasEntry, AliasTable, SchemaError, SchemaInterpreter, default_tctx, interpret,
};

/// Re-export the prelude environment and typing context constructors.
pub use handl_kit_type::primitives::{prelude_env, prelude_tctx};

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

/// Typecheck a JSON Schema by interpreting it into a `handl-kit-type` `Term`
/// and synthesising its type using the schema bridge's default primitive
/// typing context.
///
/// The `schema` is interpreted with itself as the root document, so
/// local `$defs` are resolved from it.
pub fn typecheck_schema(
    schema: &serde_json::Value,
) -> Result<handl_kit_type::Value, handl_kit_type::TypeError> {
    let term = handl_plugin_vm_type_schema_json::interpret(schema, schema)
        .map_err(|e| handl_kit_type::TypeError::CannotSynthesize(e.to_string()))?;
    let mut ctx = handl_kit_type::MetaContext::new();
    handl_kit_type::synth_type(
        &mut ctx,
        &handl_kit_type::Env::new(),
        &handl_plugin_vm_type_schema_json::default_tctx(),
        &term,
    )
}

#[cfg(test)]
mod tests;
