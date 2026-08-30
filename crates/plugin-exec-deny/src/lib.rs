//! # handl-plugin-exec-deny
//!
//! `ExecControl` rule that blocklists function calls.
//!
//! Ported from `DenyHost` in `plugin-vm-common/src/middleware.rs`. The
//! `before_eval` hook inspects `"call"` AST nodes and aborts with
//! [`EvalError::ResourceLimit`] if the callee name is in the configured
//! blocklist. Non-call nodes pass through unchecked.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, EvalError, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use handl_plugin_exec_logging::callee_name;

/// Exec-control rule that blocks calls to denylisted functions.
///
/// `before_eval` checks `"call"` nodes: if the callee name is in `denied`,
/// evaluation is aborted. Non-call nodes are always permitted.
#[derive(Debug, Clone)]
pub struct DenyControl {
    denied: HashSet<String>,
}

impl DenyControl {
    /// Create a new `DenyControl` blocking the given function names.
    pub fn new(denied: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            denied: denied.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl ExecControl for DenyControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        if let Some(name) = callee_name(ast) {
            if self.denied.contains(&name) {
                return Err(EvalError::ResourceLimit(format!("call denied: {name}")).into());
            }
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`DenyControl`] to the [`BasicEvaluator`].
pub struct ExecDenyPlugin {
    denied: HashSet<String>,
    registered: AtomicBool,
}

impl ExecDenyPlugin {
    /// Create a new `ExecDenyPlugin` blocking the given names.
    pub fn new(denied: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            denied: denied.into_iter().map(|s| s.into()).collect(),
            registered: AtomicBool::new(false),
        }
    }
}

impl Plugin for ExecDenyPlugin {
    fn id(&self) -> &str {
        "exec.deny"
    }

    fn name(&self) -> &str {
        "exec-deny"
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
                Arc::new(DenyControl::new(self.denied.iter().cloned())) as Arc<dyn ExecControl>,
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
        BasicEvaluator, Behavior, CapabilityId, CapabilityRequest, Eval, EvalBehavior,
        EvalRequest, KitRegistry, PluginContext, PluginRegistry, RuleAcceptor,
    };
    use serde_json::{Value, json};

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
    fn permits_non_denied_call() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(DenyControl::new(["dangerous"])));

        let result = run_eval(&evaluator, call("safe")).unwrap();
        assert_eq!(result, json!("safe"));
    }

    #[test]
    fn blocks_denied_call() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(DenyControl::new(["dangerous"])));

        let err = run_eval(&evaluator, call("dangerous")).unwrap_err();
        assert!(err.to_string().contains("denied"), "got: {err}");
    }
}
