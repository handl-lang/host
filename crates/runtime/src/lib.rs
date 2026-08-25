//! Minimal runtime and typecheck plugin traits for the handl platform.

pub mod runtime;
pub mod typecheck;

pub use runtime::{EvalOutput, RuntimePlugin};
pub use typecheck::{TypecheckedOutput, TypecheckPlugin};
