//! Unified value/proof space for the handl VM.
//!
//! This module re-exports the `handl-plugin-typecheck-nbe` primitives that make up the
//! single `Value`/`Term` space used by the platform VM.

pub use handl_plugin_typecheck_nbe::Term::{
    Array, Collection, Empty, Eq, Is, Membership, Null, Object, Refl, TypedLet,
};
pub use handl_plugin_typecheck_nbe::{
    Env, Kind, Level, Literal, MetaContext, Name, TCtx, Term, TypeError, Value, Var, check, eval,
    normalize, synth, synth_type,
};
pub use handl_plugin_typecheck_nbe::{prelude_env, prelude_tctx};

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
