//! Plugin-provided option schemas and resolved option values.
//!
//! The `OptionsPlugin` collects `OptionSchema` contributions from other plugins
//! and can resolve them into a `ResolvedOptions` map that operations query at
//! runtime.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::error::PlatformError;
use crate::plugin::{CapabilityRequest, CapabilityResponse, Plugin, PluginContext};
use crate::plugin_model::{Contribution, RuleAcceptor};

/// The scope at which an option is configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionScope {
    /// Applies to the entire host.
    Global,
    /// Applies to a loaded kit.
    Kit,
    /// Applies to a specific AST node kind or rule.
    Node,
}

/// A declaration of an option, its default value, and its allowed values.
#[derive(Debug, Clone)]
pub struct OptionSchema {
    /// Stable identifier for the option.
    pub id: String,
    /// Scope at which the option is configured.
    pub scope: OptionScope,
    /// Optional node kind or rule this option applies to.
    pub applies_to: Option<String>,
    /// Default value when no override is provided.
    pub default: Value,
    /// Allowed values for this option.
    pub possible_values: Vec<Value>,
    /// Human-readable description.
    pub description: String,
}

impl Contribution for OptionSchema {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Resolved option values, built from schema defaults and overrides.
#[derive(Debug, Clone, Default)]
pub struct ResolvedOptions {
    values: HashMap<String, Value>,
}

impl ResolvedOptions {
    /// Create an empty resolved options map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the raw resolved value for the given option id, if any.
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.values.get(id)
    }

    /// Returns the resolved value as a string slice if it is a JSON string.
    pub fn choice(&self, id: &str) -> Option<&str> {
        self.values.get(id).and_then(Value::as_str)
    }

    /// Returns `true` if the value for `rule_id` is considered enabled.
    pub fn is_enabled(&self, rule_id: &str) -> bool {
        match self.values.get(rule_id) {
            None => false,
            Some(Value::Bool(v)) => *v,
            Some(Value::String(s)) => {
                matches!(s.as_str(), "true" | "on" | "yes" | "1")
            }
            Some(Value::Number(n)) => n.as_f64().map_or(false, |f| f != 0.0),
            _ => true,
        }
    }
}

/// Plugin that accepts `OptionSchema` contributions and resolves option values.
pub struct OptionsPlugin {
    schemas: Vec<OptionSchema>,
    overrides: Mutex<HashMap<String, Value>>,
    capabilities: Vec<crate::capability::CapabilityId>,
}

impl OptionsPlugin {
    /// Create a new `OptionsPlugin` with no registered schemas.
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
            overrides: Mutex::new(HashMap::new()),
            capabilities: vec![crate::capability::CapabilityId::from("option/set")],
        }
    }

    /// Resolve the registered schemas into a `ResolvedOptions` map.
    ///
    /// Defaults are merged with any overrides set via the `option/set`
    /// capability.
    pub fn resolve(&self) -> ResolvedOptions {
        let mut values: HashMap<String, Value> = self
            .schemas
            .iter()
            .map(|schema| (schema.id.clone(), schema.default.clone()))
            .collect();
        if let Ok(overrides) = self.overrides.lock() {
            for (id, value) in overrides.iter() {
                values.insert(id.clone(), value.clone());
            }
        }
        ResolvedOptions { values }
    }
}

impl Default for OptionsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for OptionsPlugin {
    fn id(&self) -> &str {
        "options"
    }

    fn name(&self) -> &str {
        "options"
    }

    fn version(&self) -> &str {
        "0.0.0"
    }

    fn capabilities(&self) -> &[crate::capability::CapabilityId] {
        &self.capabilities
    }

    fn requires(&self) -> &[crate::capability::CapabilityId] {
        &[]
    }

    fn optional(&self) -> &[crate::capability::CapabilityId] {
        &[]
    }

    fn metadata(
        &self,
        _capability: &crate::capability::CapabilityId,
    ) -> Option<crate::capability::CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut crate::plugin_model::ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "option/set" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let id = request
            .payload
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PlatformError::InvalidMetadata("missing 'id' in option/set payload".into())
            })?;
        let value = request
            .payload
            .get("value")
            .cloned()
            .ok_or_else(|| {
                PlatformError::InvalidMetadata("missing 'value' in option/set payload".into())
            })?;
        self.overrides
            .lock()
            .map_err(|e| PlatformError::Plugin(e.to_string()))?
            .insert(id.to_string(), value);
        Ok(CapabilityResponse(Value::Null))
    }
}

impl RuleAcceptor for OptionsPlugin {
    type Rule = OptionSchema;

    fn accept(&mut self, rule: OptionSchema) -> Result<(), PlatformError> {
        self.schemas.push(rule);
        Ok(())
    }
}
