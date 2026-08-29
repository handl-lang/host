//! Runtime built-in composition.
//!
//! Provides the `Runtime` operation, `RuntimeBehavior` rules, and the
//! `RuntimeHost` `RuleAcceptor` that collects built-in function families.

use std::sync::Arc;

use handl_plugin_vm::{Context, Error};
use handl_plugin_vm_values_json::NonFunctionValue;
use serde_json::Value as JsonValue;

use crate::capability::{CapabilityId, CapabilityMetadata};
use crate::error::PlatformError;
use crate::plugin::{CapabilityRequest, CapabilityResponse, Plugin, PluginContext};
use crate::plugin_model::{Contribution, Operation, RuleAcceptor};

/// Runtime function-call operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Runtime;

impl Operation for Runtime {
    type Request = RuntimeRequest;
}

/// Request passed to `Runtime` behaviors.
#[derive(Debug, Clone, Default)]
pub struct RuntimeRequest {
    /// The function name being invoked.
    pub name: String,
    /// The pre-evaluated arguments.
    pub args: Vec<NonFunctionValue>,
    /// Optional result produced by a behavior.
    pub result: Option<NonFunctionValue>,
}

/// A rule that provides one or more runtime built-in functions.
pub trait RuntimeBehavior: Send + Sync + 'static {
    /// Stable family identifier for this rule.
    fn kind(&self) -> &str;

    /// The function names this rule provides.
    fn provides(&self) -> &[&'static str];

    /// Invoke one of the provided functions.
    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>>;
}

/// A `Contribution` that registers a runtime built-in family with a `RuntimeHost`.
pub struct RuntimeRule(pub Arc<dyn RuntimeBehavior>);

impl RuntimeRule {
    /// Wrap a runtime behavior rule for contribution.
    pub fn new(rule: Arc<dyn RuntimeBehavior>) -> Self {
        Self(rule)
    }
}

impl Contribution for RuntimeRule {
    fn id(&self) -> &str {
        self.0.kind()
    }
}

/// Host that collects `RuntimeBehavior` rules for the runtime target adapter.
#[derive(Default, Clone)]
pub struct RuntimeHost {
    /// Registered runtime rules.
    rules: Vec<Arc<dyn RuntimeBehavior>>,
}

impl RuntimeHost {
    /// Create a new `RuntimeHost` with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Returns the registered runtime rules.
    pub fn rules(&self) -> &[Arc<dyn RuntimeBehavior>] {
        &self.rules
    }
}

impl Plugin for RuntimeHost {
    fn name(&self) -> &str {
        "runtime-host"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

impl RuleAcceptor for RuntimeHost {
    type Rule = RuntimeRule;

    fn accept(&mut self, rule: RuntimeRule) -> Result<(), PlatformError> {
        self.rules.push(rule.0);
        Ok(())
    }
}

/// Returns a human-readable JSON kind label for error messages.
pub fn json_kind(v: &NonFunctionValue) -> &'static str {
    match v.inner() {
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
        JsonValue::String(_) => "string",
        JsonValue::Number(_) => "number",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Null => "null",
    }
}

/// Returns an error if the argument count does not match `n`.
pub fn expect_n(args: &[NonFunctionValue], n: usize) -> Result<(), Error<NonFunctionValue>> {
    if args.len() != n {
        Err(Error::ArgumentCount {
            expected: n,
            got: args.len(),
        })
    } else {
        Ok(())
    }
}

/// Extracts a number from a runtime value, or returns a type-mismatch error.
pub fn as_number(v: &NonFunctionValue) -> Result<f64, Error<NonFunctionValue>> {
    v.as_number().ok_or(Error::TypeMismatch {
        expected: "number",
        got: json_kind(v),
    })
}

/// Extracts a boolean from a runtime value, or returns a type-mismatch error.
pub fn as_bool(v: &NonFunctionValue) -> Result<bool, Error<NonFunctionValue>> {
    v.as_bool().ok_or(Error::TypeMismatch {
        expected: "boolean",
        got: json_kind(v),
    })
}
