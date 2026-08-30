//! Generic evaluation extension point.
//!
//! `handl-plugin-ext-eval` re-exports the generic `BasicEvaluator` and `Eval`
//! operation types from `handl-platform`, plus the evaluation context,
//! exec-control, and error types needed by concrete evaluation rules.
//!
//! Concrete evaluation rules (e.g. `hello.greet`, JSON value parsing) live in
//! `plugin-eval-*` crates that are registered by the kit.

pub use handl_platform::{
    BasicExecControl, BasicEvaluator, BasicEvaluatorCallback, ControlFlow, Eval, EvalBehavior,
    EvalContext, EvalError, EvalLimits, EvalRequest, EvaluatorCallback, ExecControl, ExprEvaluator, Scope,
};
