//! Minimal runtime and typecheck plugin traits for the handl platform.

pub mod runtime;
pub mod typecheck;

pub use runtime::{EvalOutput, RuntimePlugin};
pub use typecheck::{TypecheckedOutput, TypecheckPlugin};
pub use handl_platform::runtime_host::{
    as_bool, as_number, expect_n, json_kind, RuntimeBehavior, RuntimeHost, RuntimeRule,
};
