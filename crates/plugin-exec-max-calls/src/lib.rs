//! # handl-plugin-exec-max-calls
//!
//! `ExecControl` rule that bounds the total number of dispatch calls.
//!
//! Ported from `with_max_calls` in `plugin-vm-common/src/sandbox.rs`. The
//! `before_eval` hook increments `ctx.call_count` (via
//! [`EvalContext::bump_call_count`]) and returns [`EvalError::ResourceLimit`]
//! if the count exceeds the configured maximum.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, EvalError, ExecControl, PlatformError, Plugin,
    PluginContext,
};

/// Exec-control rule that enforces a maximum total dispatch-call count.
///
/// `before_eval` is called before every dispatch. It increments the context's
/// call counter and aborts with [`EvalError::ResourceLimit`] once the count
/// exceeds `max_calls`.
#[derive(Debug, Clone, Copy)]
pub struct MaxCallsControl {
    /// The maximum allowed number of dispatch calls.
    pub max_calls: usize,
}

impl MaxCallsControl {
    /// Create a new `MaxCallsControl` with the given maximum call count.
    pub fn new(max_calls: usize) -> Self {
        Self { max_calls }
    }
}

impl ExecControl for MaxCallsControl {
    fn before_eval(
        &self,
        _ast: &mut AstNode,
        ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        let count = ctx.bump_call_count();
        if count > self.max_calls {
            return Err(EvalError::ResourceLimit(format!(
                "max-calls exceeded: calls {} > limit {}",
                count, self.max_calls
            ))
            .into());
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`MaxCallsControl`] to the [`BasicEvaluator`].
pub struct ExecMaxCallsPlugin {
    max_calls: usize,
    registered: AtomicBool,
}

impl ExecMaxCallsPlugin {
    /// Create a new `ExecMaxCallsPlugin` enforcing the given call limit.
    pub fn new(max_calls: usize) -> Self {
        Self {
            max_calls,
            registered: AtomicBool::new(false),
        }
    }
}

impl Plugin for ExecMaxCallsPlugin {
    fn id(&self) -> &str {
        "exec.max-calls"
    }

    fn name(&self) -> &str {
        "exec-max-calls"
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
            ctx.contribute_exec_control(Arc::new(MaxCallsControl::new(self.max_calls))
                as Arc<dyn ExecControl>)?;
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
        BasicEvaluator, Behavior, CapabilityId, CapabilityRequest, Eval, EvalBehavior,
        EvalRequest, KitRegistry, PluginContext, PluginRegistry, RuleAcceptor,
    };
    use serde_json::{Value, json};

    /// A rule that recurses into a child once. Payload: `{"child": <ast>}`.
    struct Recurse;
    impl Behavior<Eval> for Recurse {
        fn kind(&self) -> &str {
            "recurse"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let child = req.ast.payload.get("child").cloned().unwrap_or(Value::Null);
            let child_ast: AstNode =
                serde_json::from_value(child).map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&child_ast)?;
            req.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for Recurse {}

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

    fn recurse(child: Value) -> Value {
        json!({ "kind": "recurse", "child": child })
    }

    fn value(n: i64) -> Value {
        json!({ "kind": "value", "span": null, "payload": { "value": n } })
    }

    #[test]
    fn allows_calls_within_limit() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Recurse) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>).unwrap();
        // 2 calls: root recurse + child value.
        evaluator.add_exec_control(Arc::new(MaxCallsControl::new(2)));

        let result = run_eval(&evaluator, recurse(value(42))).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn aborts_when_call_count_exceeds_limit() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Recurse) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>).unwrap();
        // Limit of 1: root recurse is call 1 (ok), child value is call 2 > 1.
        evaluator.add_exec_control(Arc::new(MaxCallsControl::new(1)));

        let err = run_eval(&evaluator, recurse(value(42))).unwrap_err();
        assert!(err.to_string().contains("max-calls"), "got: {err}");
    }
}
