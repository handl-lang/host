//! Generic evaluation extension point.
//!
//! `handl-ext-eval` re-exports the generic `BasicEvaluator` and `Eval`
//! operation types from `handl-platform`. Concrete evaluation rules
//! (e.g. `hello.greet`, JSON value parsing) live in `plugin-eval-*`
//! crates that are registered by the kit.

pub use handl_platform::{BasicEvaluator, Eval, EvalBehavior, EvalRequest};
