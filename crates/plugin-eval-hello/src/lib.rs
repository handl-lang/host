//! `hello.greet` evaluation rule for the generic `BasicEvaluator`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalRequest, PlatformError,
    Plugin, PluginContext,
};
use handl_plugin_lib_hello::greet;

/// Evaluation rule that handles `hello.greet(...)` expressions.
#[derive(Debug, Default, Clone, Copy)]
pub struct HelloGreet;

impl Behavior<Eval> for HelloGreet {
    fn kind(&self) -> &str {
        "hello.greet"
    }

    fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
        let name = if let Some(name) = op.ast.payload.get("name").and_then(|v| v.as_str()) {
            name.to_string()
        } else if let Some(arg) = op
            .ast
            .payload
            .get("args")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
        {
            parse_greet_arg(arg).unwrap_or_else(|| "world".to_string())
        } else {
            "world".to_string()
        };

        op.result = Some(serde_json::Value::String(greet(&name)));
        Ok(())
    }
}

impl EvalBehavior for HelloGreet {}

fn parse_greet_arg(arg: &str) -> Option<String> {
    let rest = arg.strip_prefix("hello.greet")?;
    let rest = rest.trim();
    let inner = rest.strip_prefix("(")?.strip_suffix(")")?;
    let inner = inner.trim();
    let quoted = inner.strip_prefix('"')?.strip_suffix('"')?;
    Some(quoted.to_string())
}

/// Registers the `hello.greet` rule with the `BasicEvaluator`.
pub struct HelloEvalPlugin {
    registered: AtomicBool,
}

impl HelloEvalPlugin {
    /// Create a new `HelloEvalPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for HelloEvalPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelloEvalPlugin {
    fn id(&self) -> &str {
        "eval.hello"
    }

    fn name(&self) -> &str {
        "eval-hello"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(HelloGreet) as Arc<dyn EvalBehavior>)?;
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
