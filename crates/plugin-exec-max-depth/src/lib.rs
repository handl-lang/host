//! # handl-plugin-exec-exec-max-depth
//!
//! `ExecControl` rule that bounds recursion (scope-stack) depth.
//!
//! Ported from `with_max_depth` in `plugin-vm-common/src/sandbox.rs`. The
//! `before_eval` hook checks `ctx.depth()` against a configured maximum and
//! returns [`EvalError::ResourceLimit`] if the limit would be exceeded.
//!
//! Unlike the context-level `EvalLimits::max_depth` (which is checked by
//! `EvalContext::check_depth` when a rule explicitly pushes a scope), this
//! plugin gates **every dispatch** — so even a deeply nested chain of
//! non-scope-pushing rules is bounded.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, EvalError, ExecControl, PlatformError, Plugin,
    PluginContext,
};

/// Exec-control rule that enforces a maximum scope-stack depth.
///
/// `before_eval` is called before every dispatch. If the current
/// [`EvalContext::depth`] is greater than or equal to `max_depth`, evaluation
/// is aborted with [`EvalError::ResourceLimit`].
#[derive(Debug, Clone, Copy)]
pub struct MaxDepthControl {
    /// The maximum allowed scope-stack depth (global scope is depth 1).
    pub max_depth: usize,
}

impl MaxDepthControl {
    /// Create a new `MaxDepthControl` with the given maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }
}

impl ExecControl for MaxDepthControl {
    fn before_eval(
        &self,
        _ast: &mut AstNode,
        ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        if ctx.depth() > self.max_depth {
            return Err(EvalError::ResourceLimit(format!(
                "max-depth exceeded: depth {} > limit {}",
                ctx.depth(),
                self.max_depth
            ))
            .into());
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`MaxDepthControl`] to the [`BasicEvaluator`].
pub struct ExecMaxDepthPlugin {
    max_depth: usize,
    registered: AtomicBool,
}

impl ExecMaxDepthPlugin {
    /// Create a new `ExecMaxDepthPlugin` enforcing the given maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            registered: AtomicBool::new(false),
        }
    }
}

impl Plugin for ExecMaxDepthPlugin {
    fn id(&self) -> &str {
        "exec.max-depth"
    }

    fn name(&self) -> &str {
        "exec-max-depth"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if !self.registered.load(Ordering::SeqCst) {
            self.registered.store(true, Ordering::SeqCst);
            ctx.contribute_exec_control(
                Arc::new(MaxDepthControl::new(self.max_depth)) as Arc<dyn ExecControl>
            )?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        BasicEvaluator, Behavior, CapabilityId, CapabilityRequest, Eval, EvalBehavior, EvalRequest,
        KitRegistry, PluginContext, PluginRegistry, RuleAcceptor,
    };
    use serde_json::{Value, json};

    /// A rule that pushes a scope and recurses into a child, simulating
    /// nested evaluation. Payload: `{"child": <ast>}`.
    struct Nest;
    impl Behavior<Eval> for Nest {
        fn kind(&self) -> &str {
            "nest"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            req.ctx.push_scope();
            let child = req.ast.payload.get("child").cloned().unwrap_or(Value::Null);
            let child_ast: AstNode =
                serde_json::from_value(child).map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&child_ast);
            req.ctx.pop_scope();
            req.result = Some(result?);
            Ok(())
        }
    }
    impl EvalBehavior for Nest {}

    /// A leaf rule returning a literal value.
    struct ValueLit;
    impl Behavior<Eval> for ValueLit {
        fn kind(&self) -> &str {
            "value"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            req.result = Some(req.ast.payload.get("value").cloned().unwrap_or(Value::Null));
            Ok(())
        }
    }
    impl EvalBehavior for ValueLit {}

    fn run_eval(evaluator: &BasicEvaluator, payload: Value) -> Result<Value, PlatformError> {
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator.run(
            &CapabilityRequest {
                capability: CapabilityId::from("eval"),
                payload,
            },
            &mut pctx,
        )?;
        Ok(resp.0)
    }

    fn nest(child: Value) -> Value {
        json!({ "kind": "nest", "child": child })
    }

    fn value(n: i64) -> Value {
        json!({ "kind": "value", "span": null, "payload": { "value": n } })
    }

    #[test]
    fn allows_evaluation_within_limit() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Nest) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(Arc::new(MaxDepthControl::new(5)));

        // nest(value(1)) -> depth goes 1 -> 2 (push) -> eval value. Within limit 5.
        let result = run_eval(&evaluator, nest(value(1))).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn aborts_when_depth_exceeds_limit() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Nest) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        // Limit of 1: the root dispatch is at depth 1, which is allowed, but
        // any nesting pushes to depth 2 > 1.
        evaluator.add_exec_control(Arc::new(MaxDepthControl::new(1)));

        let err = run_eval(&evaluator, nest(value(1))).unwrap_err();
        assert!(err.to_string().contains("max-depth"), "got: {err}");
    }
}
