//! Plugin integration: `Refine` operation, `RefinementBehavior` trait,
//! `BasicRefiner` rule acceptor, and `RefinementCore` plugin.
//!
//! This mirrors the `BasicTypechecker` pattern in `crates/platform/src/typecheck.rs`:
//! - `Refine` is the operation marker (like `Typecheck`).
//! - `RefineRequest` / `RefineResult` are the request/response types.
//! - `RefinementBehavior` is the behavior trait (like `TypecheckBehavior`).
//! - `BasicRefiner` is the `RuleAcceptor` plugin (like `BasicTypechecker`).
//! - `RefinementCore` is a `Plugin` that contributes the canonical refinement rule.

use serde_json::Value;

use handl_platform::{
    Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, PlatformError, Plugin, PluginContext, ResolvedOptions, RuleAcceptor,
};

use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::RefinementResult;

/// Refine operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Refine;

impl Operation for Refine {
    type Request = RefineRequest;
}

/// Request passed to `Refine` behaviors.
#[derive(Debug, Clone)]
pub struct RefineRequest {
    /// The serialized `Scheme` to refine (as JSON, matching `Scheme`'s serde model).
    pub scheme_json: Value,
    /// Resolved refiner options.
    pub options: ResolvedOptions,
}

/// Result of a refinement operation.
#[derive(Debug, Clone)]
pub struct RefineResult {
    /// The refinement result.
    pub result: Result<RefinementResult, String>,
}

/// Behavior trait for refining a specific operation kind.
pub trait RefinementBehavior: Behavior<Refine> {}

impl Contribution for Box<dyn RefinementBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Refine` operation.
///
/// Like `BasicTypechecker`, this has no knowledge of concrete refinement logic;
/// it dispatches to registered `RefinementBehavior` rules keyed by operation kind.
pub struct BasicRefiner {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered refinement rules.
    pub rules: Vec<Box<dyn RefinementBehavior>>,
    /// Option schemas contributed for the refiner.
    pub option_schemas: Vec<handl_platform::OptionSchema>,
}

impl BasicRefiner {
    /// Create a new `BasicRefiner` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("refine")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicRefiner {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicRefiner {
    fn id(&self) -> &str {
        "refine.basic"
    }

    fn name(&self) -> &str {
        "Basic refiner"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "refine").then(|| {
            serde_json::json!({
                "command": "refine",
                "description": "Refine a Handl scheme (annotation-free refinement inference)"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "refine" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("refine");

        let mut req = RefineRequest {
            scheme_json: request.payload.clone(),
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                if let Err(err) = rule.apply(&mut req) {
                    return Ok(CapabilityResponse(serde_json::json!({
                        "refined": false,
                        "error": err.to_string()
                    })));
                }
            }
        }

        Ok(CapabilityResponse(serde_json::json!({"refined": true})))
    }
}

impl RuleAcceptor for BasicRefiner {
    type Rule = Box<dyn RefinementBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}

/// The canonical refinement core plugin.
///
/// This plugin provides the `refine` capability and runs the annotation-free
/// refinement inference engine on a `Scheme` (passed as JSON in the request
/// payload). It uses the standard `ContractRegistry`.
pub struct RefinementCore {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// The contract registry used for inference.
    registry: ContractRegistry,
}

impl RefinementCore {
    /// Create a new `RefinementCore` with the standard contract registry.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("refine")],
            registry: ContractRegistry::standard(),
        }
    }

    /// Create a new `RefinementCore` with a custom contract registry.
    pub fn with_registry(registry: ContractRegistry) -> Self {
        Self {
            capabilities: vec![CapabilityId::from("refine")],
            registry,
        }
    }

    /// Run refinement inference on a `Scheme`.
    pub fn refine(&self, scheme: &handl_language::Scheme) -> Result<RefinementResult, String> {
        infer_refinement(scheme, &self.registry)
    }
}

impl Default for RefinementCore {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for RefinementCore {
    fn id(&self) -> &str {
        "refine.core"
    }

    fn name(&self) -> &str {
        "Refinement core"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "refine").then(|| {
            serde_json::json!({
                "command": "refine",
                "description": "Annotation-free refinement inference for Handl schemes"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "refine" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        // The payload is expected to contain a serialized Scheme.
        // We deserialize it and run inference.
        let scheme: handl_language::Scheme =
            serde_json::from_value(request.payload.clone()).map_err(|e| {
                PlatformError::InvalidMetadata(format!("invalid scheme payload: {}", e))
            })?;

        match self.refine(&scheme) {
            Ok(result) => {
                let guarantee = result.guarantee.pretty();
                let input = result.input.pretty();
                let output = result.output.pretty();
                let loop_inv_count = result.loop_invariants.len();
                let termination = result
                    .termination
                    .as_ref()
                    .map(|m| m.pretty())
                    .unwrap_or_else(|| "N/A".to_string());

                Ok(CapabilityResponse(serde_json::json!({
                    "refined": true,
                    "input": input,
                    "output": output,
                    "guarantee": guarantee,
                    "loop_invariants": loop_inv_count,
                    "termination": termination,
                })))
            }
            Err(e) => Ok(CapabilityResponse(serde_json::json!({
                "refined": false,
                "error": e
            }))),
        }
    }
}
