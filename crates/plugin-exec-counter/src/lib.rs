//! # handl-plugin-exec-counter
//!
//! `ExecControl` rule that counts completed dispatches.
//!
//! Ported from `Counter` in `plugin-vm-common/src/middleware_vm.rs`. The
//! `before_eval` hook increments an internal [`AtomicUsize`] counter; the
//! count is exposed via [`CounterControl::count`] or the shared
//! [`CounterControl::counter`] handle.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, ExecControl, PlatformError, Plugin,
    PluginContext,
};

/// Exec-control rule that counts every dispatch.
///
/// The counter is shared via an [`Arc<AtomicUsize>`] so it can be read after
/// evaluation completes. `before_eval` increments the counter (so the count
/// reflects the number of dispatches attempted, including ones that later
/// error).
#[derive(Debug, Clone, Default)]
pub struct CounterControl {
    count: Arc<AtomicUsize>,
}

impl CounterControl {
    /// Create a new counter starting at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a clone of the shared counter handle.
    pub fn counter(&self) -> Arc<AtomicUsize> {
        self.count.clone()
    }

    /// Returns the current count.
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

impl ExecControl for CounterControl {
    fn before_eval(
        &self,
        _ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`CounterControl`] to the [`BasicEvaluator`].
pub struct ExecCounterPlugin {
    counter: CounterControl,
    registered: AtomicBool,
}

impl ExecCounterPlugin {
    /// Create a new `ExecCounterPlugin`. The shared counter handle can be
    /// retrieved via [`counter`](Self::counter) after contribution.
    pub fn new() -> Self {
        Self {
            counter: CounterControl::new(),
            registered: AtomicBool::new(false),
        }
    }

    /// Returns a clone of the shared counter control.
    pub fn counter(&self) -> CounterControl {
        self.counter.clone()
    }
}

impl Default for ExecCounterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExecCounterPlugin {
    fn id(&self) -> &str {
        "exec.counter"
    }

    fn name(&self) -> &str {
        "exec-counter"
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
            ctx.contribute_exec_control(Arc::new(self.counter.clone())
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

    #[test]
    fn counts_every_dispatch() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Recurse) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>).unwrap();
        let counter = CounterControl::new();
        evaluator.add_exec_control(Arc::new(counter.clone()));

        let payload = json!({
            "kind": "recurse",
            "child": { "kind": "value", "span": null, "payload": { "value": 1 } }
        });
        let _ = run_eval(&evaluator, payload).unwrap();
        assert_eq!(counter.count(), 2, "two dispatches should be counted");
    }

    #[test]
    fn shared_counter_handle_reflects_count() {
        let counter = CounterControl::new();
        let handle = counter.counter();
        assert_eq!(handle.load(Ordering::Relaxed), 0);
        // Simulate increment via the control by constructing a minimal ctx.
        let mut ctx = handl_platform::EvalContext::new();
        let mut ast = AstNode {
            kind: "x".into(),
            span: None,
            payload: Value::Null,
        };
        let _ = ExecControl::before_eval(&counter, &mut ast, &mut ctx);
        assert_eq!(handle.load(Ordering::Relaxed), 1);
    }
}
