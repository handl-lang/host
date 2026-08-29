//! JSON value evaluation rule for the generic `BasicEvaluator`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalRequest, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

/// Evaluation rule that parses a JSON literal string.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonEval;

impl Behavior<Eval> for JsonEval {
    fn kind(&self) -> &str {
        "json"
    }

    fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
        let expr = expr_text(&op.ast.payload)
            .ok_or_else(|| PlatformError::InvalidMetadata("missing expr or args".into()))?;

        let value: Value = serde_json::from_str(&expr)
            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;

        op.result = Some(normalize_number(value));
        Ok(())
    }
}

impl EvalBehavior for JsonEval {}

fn expr_text(payload: &Value) -> Option<String> {
    if let Some(expr) = payload.get("expr").and_then(Value::as_str) {
        return Some(expr.to_string());
    }

    if let Some(args) = payload.get("args").and_then(Value::as_array) {
        let joined: Vec<&str> = args.iter().filter_map(Value::as_str).collect();
        if !joined.is_empty() {
            return Some(joined.join(" "));
        }
    }

    None
}

fn normalize_number(value: Value) -> Value {
    match value {
        Value::Number(n) => Value::Number(
            serde_json::Number::from_f64(n.as_f64().unwrap_or_default()).unwrap_or(n),
        ),
        _ => value,
    }
}

/// Registers the JSON evaluation rule with the `BasicEvaluator`.
pub struct JsonEvalPlugin {
    registered: AtomicBool,
}

impl JsonEvalPlugin {
    /// Create a new `JsonEvalPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for JsonEvalPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for JsonEvalPlugin {
    fn id(&self) -> &str {
        "eval.json"
    }

    fn name(&self) -> &str {
        "eval-json"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(JsonEval) as Arc<dyn EvalBehavior>)?;
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
