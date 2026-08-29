//! Minimal runtime and typecheck plugin traits for the handl platform.

pub mod runtime;
pub mod typecheck;

pub use handl_platform::runtime_host::{
    RuntimeBehavior, RuntimeHost, RuntimeRule, as_bool, as_number, expect_n, json_kind,
};
pub use runtime::{EvalOutput, RuntimePlugin};
pub use typecheck::{TypecheckPlugin, TypecheckedOutput};
