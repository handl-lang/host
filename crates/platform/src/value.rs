//! Unified value/proof space for the handl VM.
//!
//! This module re-exports the `handl-kit-type` primitives that make up the
//! single `Value`/`Term` space used by the platform VM.

pub use handl_plugin_lib_type::Term::{
    Array, Collection, Empty, Eq, Is, Membership, Null, Object, Refl, TypedLet,
};
pub use handl_plugin_lib_type::{
    check, eval, normalize, synth, synth_type, Env, Kind, Level, Literal, MetaContext, Name, TCtx,
    Term, TypeError, Value, Var,
};
pub use handl_plugin_lib_type::{prelude_env, prelude_tctx, typecheck, typecheck_with_prelude};
