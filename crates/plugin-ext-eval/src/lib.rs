//! Generic evaluation extension point.
//!
//! `handl-plugin-ext-eval` re-exports the generic `BasicEvaluator` and `Eval`
//! operation types from `handl-platform`, plus the evaluation context,
//! exec-control, and error types needed by concrete evaluation rules.
//!
//! It also provides function-registration adapters ([`GenericFunction`],
//! [`LazyFunction`], [`AsyncFn`]) — the ADR 0040 replacements for
//! `plugin-vm-common`'s `builder.rs` closure adapters.
//!
//! Concrete evaluation rules (e.g. `hello.greet`, JSON value parsing) live in
//! `plugin-eval-*` crates that are registered by the kit.

pub use handl_platform::{
    AsyncFunction, BasicEvaluator, BasicEvaluatorCallback, BasicExecControl, ControlFlow, Eval,
    EvalBehavior, EvalContext, EvalError, EvalLimits, EvalRequest, EvaluatorCallback, ExecControl,
    ExprEvaluator, Function, HandlerClause, Module, ModuleLookup, Scope,
};

pub mod adapters;
pub use adapters::{AsyncFn, EvalFuture, GenericFunction, LazyFunction};
