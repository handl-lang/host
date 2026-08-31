//! # handl-plugin-exec-allow
//!
//! `ExecControl` rule that allowlists function calls.
//!
//! Ported from `AllowHost` in `plugin-vm-common/src/middleware.rs`. The
//! `before_eval` hook inspects `"call"` AST nodes and aborts with
//! [`EvalError::ResourceLimit`] if the callee name is not in the configured
//! allowlist. Non-call nodes pass through unchecked.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, EvalError, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use handl_plugin_exec_logging::callee_name;

/// Exec-control rule that only permits calls to allowlisted functions.
///
/// `before_eval` checks `"call"` nodes: if the callee name is not in
/// `allowed`, evaluation is aborted. Non-call nodes are always permitted.
#[derive(Debug, Clone)]
pub struct AllowControl {
    allowed: HashSet<String>,
}

impl AllowControl {
    /// Create a new `AllowControl` permitting only the given function names.
    pub fn new(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed: allowed.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl ExecControl for AllowControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        if let Some(name) = callee_name(ast) {
            if !self.allowed.contains(&name) {
                return Err(EvalError::ResourceLimit(format!("call not allowed: {name}")).into());
            }
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`AllowControl`] to the [`BasicEvaluator`].
pub struct ExecAllowPlugin {
    allowed: HashSet<String>,
    registered: AtomicBool,
}

impl ExecAllowPlugin {
    /// Create a new `ExecAllowPlugin` permitting only the given names.
    pub fn new(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed: allowed.into_iter().map(|s| s.into()).collect(),
            registered: AtomicBool::new(false),
        }
    }
}

impl Plugin for ExecAllowPlugin {
    fn id(&self) -> &str {
        "exec.allow"
    }

    fn name(&self) -> &str {
        "exec-allow"
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
                Arc::new(AllowControl::new(self.allowed.iter().cloned())) as Arc<dyn ExecControl>
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

    /// A "call" rule that just returns the callee name (for testing gating).
    struct CallEcho;
    impl Behavior<Eval> for CallEcho {
        fn kind(&self) -> &str {
            "call"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let name = handl_plugin_exec_logging::callee_name(&req.ast)
                .unwrap_or_else(|| "<unknown>".to_string());
            req.result = Some(Value::String(name));
            Ok(())
        }
    }
    impl EvalBehavior for CallEcho {}

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

    fn call(name: &str) -> Value {
        json!({
            "kind": "call",
            "callee": { "kind": "var", "span": null, "payload": { "name": name } },
            "args": []
        })
    }

    #[test]
    fn permits_allowlisted_call() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(Arc::new(AllowControl::new(["math.add"])));

        let result = run_eval(&evaluator, call("math.add")).unwrap();
        assert_eq!(result, json!("math.add"));
    }

    #[test]
    fn blocks_non_allowlisted_call() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(Arc::new(AllowControl::new(["math.add"])));

        let err = run_eval(&evaluator, call("dangerous")).unwrap_err();
        assert!(err.to_string().contains("not allowed"), "got: {err}");
    }
}
