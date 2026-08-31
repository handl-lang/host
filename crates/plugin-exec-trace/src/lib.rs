//! # handl-plugin-exec-trace
//!
//! `ExecControl` rule that records every evaluation for observability.
//!
//! Ported from `Trace` in `plugin-vm-common/src/middleware_vm.rs`. The
//! `before_eval` hook logs the AST kind being dispatched (to stderr) and the
//! `after_eval` hook records a [`TraceEvent`] with the kind and result. The
//! recorded events are accessible via [`TraceControl::events`].

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

/// A recorded evaluation event.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// The `AstNode.kind` that was dispatched.
    pub kind: String,
    /// The result produced (or `None` if the dispatch was skipped/errored
    /// before `after_eval` ran with a result).
    pub result: Option<Value>,
}

/// Exec-control rule that traces every dispatch.
///
/// `before_eval` prints `[trace] before <kind>` to stderr; `after_eval`
/// appends a [`TraceEvent`] to an internal buffer. The buffer is shared via
/// an `Arc<Mutex<...>>` so the same trace can be inspected after evaluation
/// completes.
#[derive(Clone, Default)]
pub struct TraceControl {
    events: Arc<Mutex<Vec<TraceEvent>>>,
}

impl TraceControl {
    /// Create a new trace recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a clone of the recorded trace events.
    pub fn events(&self) -> Vec<TraceEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the number of recorded events.
    pub fn len(&self) -> usize {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Returns `true` if no events have been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the recorded events.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.events.lock() {
            guard.clear();
        }
    }
}

impl std::fmt::Debug for TraceControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraceControl")
            .field("len", &self.len())
            .finish()
    }
}

impl ExecControl for TraceControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        eprintln!("[trace] before {}", ast.kind);
        Ok(ControlFlow::Continue)
    }

    fn after_eval(
        &self,
        ast: &AstNode,
        result: &Value,
        _ctx: &mut EvalContext,
    ) -> Result<(), PlatformError> {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(TraceEvent {
                kind: ast.kind.clone(),
                result: Some(result.clone()),
            });
        }
        Ok(())
    }
}

/// Plugin that contributes [`TraceControl`] to the [`BasicEvaluator`].
pub struct ExecTracePlugin {
    trace: TraceControl,
    registered: AtomicBool,
}

impl ExecTracePlugin {
    /// Create a new `ExecTracePlugin`. The shared [`TraceControl`] handle
    /// can be retrieved via [`trace`](Self::trace) after contribution to
    /// inspect recorded events.
    pub fn new() -> Self {
        Self {
            trace: TraceControl::new(),
            registered: AtomicBool::new(false),
        }
    }

    /// Returns a handle to the shared trace recorder.
    pub fn trace(&self) -> TraceControl {
        self.trace.clone()
    }
}

impl Default for ExecTracePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExecTracePlugin {
    fn id(&self) -> &str {
        "exec.trace"
    }

    fn name(&self) -> &str {
        "exec-trace"
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
            ctx.contribute_exec_control(Arc::new(self.trace.clone()) as Arc<dyn ExecControl>)?;
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

    /// A rule that recurses into a child.
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
    fn records_one_event_per_dispatch() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Recurse) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        let trace = TraceControl::new();
        evaluator.add_exec_control(Arc::new(trace.clone()));

        let payload = json!({
            "kind": "recurse",
            "child": { "kind": "value", "span": null, "payload": { "value": 7 } }
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(7));

        let events = trace.events();
        assert_eq!(events.len(), 2, "two dispatches should be traced");
        // after_eval runs in LIFO order (innermost dispatch first): the child
        // "value" completes before the outer "recurse".
        assert_eq!(events[0].kind, "value");
        assert_eq!(events[1].kind, "recurse");
        assert_eq!(events[0].result, Some(json!(7)));
    }

    #[test]
    fn trace_is_independent_until_shared() {
        let trace = TraceControl::new();
        assert!(trace.is_empty());
        assert_eq!(trace.len(), 0);
    }
}
