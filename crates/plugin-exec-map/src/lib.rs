//! # handl-plugin-exec-map
//!
//! `ExecControl` rule that rewrites function names in call expressions.
//!
//! Ported from `MapHost` in `plugin-vm-common/src/middleware.rs`. The
//! `before_eval` hook inspects `"call"` AST nodes and rewrites the callee
//! name through a configured mapper closure (e.g. prefixing `"add"` →
//! `"math.add"` or stripping a namespace). Because `before_eval` receives a
//! `&mut AstNode`, the rewrite happens in place before dispatch.
//!
//! The call payload is `{"callee": <ast>, "args": [...]}`; the callee is
//! typically a `"var"` node whose payload carries `{"name": "<fn>"}`. The
//! map rule rewrites that `"name"` field.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

/// Exec-control rule that rewrites call callee names through a mapper.
///
/// The mapper is a `Box<dyn Fn(&str) -> String + Send + Sync>`. For `"call"`
/// nodes whose callee is a `"var"` with a `"name"` field, the name is
/// replaced with `mapper(name)`. Non-call nodes and unrecognised callee
/// shapes are left untouched.
pub struct MapControl {
    mapper: Box<dyn Fn(&str) -> String + Send + Sync>,
}

impl MapControl {
    /// Create a new `MapControl` with a custom mapper closure.
    pub fn new(mapper: impl Fn(&str) -> String + Send + Sync + 'static) -> Self {
        Self {
            mapper: Box::new(mapper),
        }
    }

    /// Convenience constructor that prepends a fixed prefix to every name.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        Self::new(move |name| format!("{prefix}{name}"))
    }

    /// Convenience constructor that strips a fixed prefix from every name.
    pub fn strip_prefix(prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        Self::new(move |name| name.strip_prefix(&prefix).unwrap_or(name).to_string())
    }
}

impl std::fmt::Debug for MapControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapControl").finish_non_exhaustive()
    }
}

impl ExecControl for MapControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        if ast.kind != "call" {
            return Ok(ControlFlow::Continue);
        }
        // The callee lives under payload["callee"] as a serialized AstNode.
        // We rewrite it in place: deserialize, map the var name, re-serialize.
        let Some(callee) = ast.payload.get("callee").cloned() else {
            return Ok(ControlFlow::Continue);
        };
        let Ok(mut callee_ast) = serde_json::from_value::<AstNode>(callee) else {
            return Ok(ControlFlow::Continue);
        };
        if callee_ast.kind == "var" {
            if let Some(name) = callee_ast.payload.get("name").and_then(Value::as_str) {
                let mapped = (self.mapper)(name);
                if let Some(obj) = callee_ast.payload.as_object_mut() {
                    obj.insert("name".to_string(), Value::String(mapped));
                }
            }
        }
        // Write the rewritten callee back into the call payload.
        if let Some(obj) = ast.payload.as_object_mut() {
            obj.insert("callee".to_string(), serde_json::to_value(&callee_ast).unwrap_or(Value::Null));
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`MapControl`] to the [`BasicEvaluator`].
pub struct ExecMapPlugin {
    control: Arc<MapControl>,
    registered: AtomicBool,
}

impl ExecMapPlugin {
    /// Create a new `ExecMapPlugin` wrapping the given [`MapControl`].
    pub fn new(control: MapControl) -> Self {
        Self {
            control: Arc::new(control),
            registered: AtomicBool::new(false),
        }
    }

    /// Convenience constructor that prepends a fixed prefix.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self::new(MapControl::with_prefix(prefix))
    }

    /// Convenience constructor that strips a fixed prefix.
    pub fn strip_prefix(prefix: impl Into<String>) -> Self {
        Self::new(MapControl::strip_prefix(prefix))
    }
}

impl Plugin for ExecMapPlugin {
    fn id(&self) -> &str {
        "exec.map"
    }

    fn name(&self) -> &str {
        "exec-map"
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
            ctx.contribute_exec_control(self.control.clone() as Arc<dyn ExecControl>)?;
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

    /// A "call" rule that returns the callee name (so we can observe the
    /// rewrite performed by MapControl).
    struct CallEcho;
    impl Behavior<Eval> for CallEcho {
        fn kind(&self) -> &str {
            "call"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let callee = req.ast.payload.get("callee").cloned().unwrap_or(Value::Null);
            let callee_ast: AstNode =
                serde_json::from_value(callee).map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let name = callee_ast
                .payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_string();
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
    fn prefixes_callee_name() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(MapControl::with_prefix("math.")));

        let result = run_eval(&evaluator, call("add")).unwrap();
        assert_eq!(result, json!("math.add"));
    }

    #[test]
    fn strips_callee_prefix() {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(CallEcho) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(MapControl::strip_prefix("math.")));

        let result = run_eval(&evaluator, call("math.add")).unwrap();
        assert_eq!(result, json!("add"));
    }

    #[test]
    fn leaves_non_call_nodes_untouched() {
        // A non-call node should pass through; the mapper must not crash.
        let mut ast = AstNode {
            kind: "value".into(),
            span: None,
            payload: json!({ "value": 1 }),
        };
        let mut ctx = handl_platform::EvalContext::new();
        let control = MapControl::with_prefix("math.");
        let flow = ExecControl::before_eval(&control, &mut ast, &mut ctx).unwrap();
        assert_eq!(flow, ControlFlow::Continue);
        assert_eq!(ast.payload, json!({ "value": 1 }));
    }
}
