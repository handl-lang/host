//! # handl-plugin-exec-logging
//!
//! `ExecControl` rule that logs function calls.
//!
//! Ported from `LoggingHost` in `plugin-vm-common/src/middleware.rs`. The
//! `before_eval` hook inspects `"call"` AST nodes, extracts the callee name,
//! and logs it (to stderr). Non-call nodes are passed through silently.
//!
//! The call payload is `{"callee": <ast>, "args": [...]}`; the callee is
//! typically a `"var"` node whose payload carries `{"name": "<fn>"}`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

/// Extract the callee name from a `"call"` AST node, if possible.
///
/// The callee is expected to be a serialized `AstNode` under
/// `payload["callee"]`. If that node is a `"var"` with a `"name"` field, the
/// name is returned. Returns `None` for non-call nodes or unrecognised
/// callee shapes.
pub fn callee_name(ast: &AstNode) -> Option<String> {
    if ast.kind != "call" {
        return None;
    }
    let callee = ast.payload.get("callee")?;
    let callee_ast: AstNode = serde_json::from_value(callee.clone()).ok()?;
    if callee_ast.kind == "var" {
        callee_ast
            .payload
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
    } else {
        None
    }
}

/// Exec-control rule that logs function calls.
///
/// `before_eval` prints `[call] <name>` to stderr for `"call"` nodes where
/// the callee name can be extracted. Other nodes are not logged.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingControl;

impl LoggingControl {
    /// Create a new `LoggingControl`.
    pub fn new() -> Self {
        Self
    }
}

impl ExecControl for LoggingControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        if let Some(name) = callee_name(ast) {
            eprintln!("[call] {name}");
        }
        Ok(ControlFlow::Continue)
    }
}

/// Plugin that contributes [`LoggingControl`] to the [`BasicEvaluator`].
pub struct ExecLoggingPlugin {
    registered: AtomicBool,
}

impl ExecLoggingPlugin {
    /// Create a new `ExecLoggingPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ExecLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExecLoggingPlugin {
    fn id(&self) -> &str {
        "exec.logging"
    }

    fn name(&self) -> &str {
        "exec-logging"
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
            ctx.contribute_exec_control(Arc::new(LoggingControl) as Arc<dyn ExecControl>)?;
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
    use serde_json::json;

    #[test]
    fn extracts_callee_name_from_call_node() {
        let ast = AstNode {
            kind: "call".into(),
            span: None,
            payload: json!({
                "callee": { "kind": "var", "span": null, "payload": { "name": "math.add" } },
                "args": []
            }),
        };
        assert_eq!(callee_name(&ast), Some("math.add".to_string()));
    }

    #[test]
    fn returns_none_for_non_call_node() {
        let ast = AstNode {
            kind: "value".into(),
            span: None,
            payload: json!({ "value": 1 }),
        };
        assert_eq!(callee_name(&ast), None);
    }

    #[test]
    fn returns_none_when_callee_is_not_a_var() {
        let ast = AstNode {
            kind: "call".into(),
            span: None,
            payload: json!({
                "callee": { "kind": "value", "span": null, "payload": { "value": 1 } },
                "args": []
            }),
        };
        assert_eq!(callee_name(&ast), None);
    }
}
