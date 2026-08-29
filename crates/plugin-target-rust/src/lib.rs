//! Runtime plugin backed by the Rust VM interpreter.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use handl_platform::{
    AstNode, BasicEvaluator, CapabilityId, CapabilityRequest, CapabilityResponse,
    ContributionContext, EvalBehavior, EvalRequest, PlatformError, Plugin, PluginContext,
    ResolvedOptions, RuntimeBehavior, RuntimeHost,
};
use handl_platform::value::{Literal, Name, Value as RuntimeValue};
use handl_plugin_runtime::{EvalOutput, RuntimePlugin};
use serde_json::Value as JsonValue;

/// A `RuntimePlugin` adapter around the Rust VM.
pub struct RustVm {
    capabilities: Vec<CapabilityId>,
    eval_rules: Mutex<Vec<Arc<dyn EvalBehavior>>>,
    runtime_rules: Mutex<Vec<Arc<dyn RuntimeBehavior>>>,
}

impl RustVm {
    /// Creates a new `RustVm` plugin with no extra libraries.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("runtime")],
            eval_rules: Mutex::new(Vec::new()),
            runtime_rules: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RustVm {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimePlugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn eval(&self, stmts: &[AstNode]) -> Result<EvalOutput, PlatformError> {
        let rules = self.eval_rules.lock().unwrap().clone();
        let mut values = Vec::new();

        for stmt in stmts {
            let mut req = EvalRequest {
                ast: stmt.clone(),
                options: ResolvedOptions::new(),
                result: None,
            };

            for rule in &rules {
                if rule.kind() == stmt.kind {
                    rule.apply(&mut req)?;
                }
            }

            let value = match req.result {
                Some(v) => json_to_runtime(v),
                None => RuntimeValue::Null,
            };
            values.push(value);
        }

        Ok(EvalOutput { values })
    }
}

impl Plugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        Some(serde_json::json!({}))
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if let Some(evaluator) = ctx.get_plugin_mut::<BasicEvaluator>() {
            *self.eval_rules.lock().unwrap() = evaluator.rules.clone();
        }
        if let Some(host) = ctx.get_plugin_mut::<RuntimeHost>() {
            *self.runtime_rules.lock().unwrap() = host.rules().to_vec();
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

fn json_to_runtime(v: JsonValue) -> RuntimeValue {
    match v {
        JsonValue::Null => RuntimeValue::Null,
        JsonValue::Bool(b) => RuntimeValue::Literal(Literal::Bool(b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                RuntimeValue::Literal(Literal::Int(i))
            } else {
                RuntimeValue::Literal(Literal::String(n.to_string()))
            }
        }
        JsonValue::String(s) => RuntimeValue::Literal(Literal::String(s)),
        JsonValue::Array(a) => RuntimeValue::Array(a.into_iter().map(json_to_runtime).collect()),
        JsonValue::Object(o) => RuntimeValue::Object(
            o.into_iter()
                .map(|(k, v)| (Name::new(k), json_to_runtime(v)))
                .collect::<BTreeMap<_, _>>(),
        ),
    }
}
