use handl_platform::{
    Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// LSP operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Lsp;

impl Operation for Lsp {
    type Request = LspRequest;
}

/// Request passed to `Lsp` behaviors.
#[derive(Debug, Clone)]
pub struct LspRequest {
    /// LSP method name.
    pub method: String,
    /// LSP method parameters.
    pub params: serde_json::Value,
    /// Resolved LSP options.
    pub options: ResolvedOptions,
}

/// Behavior trait for handling a specific LSP method.
pub trait LspBehavior: Behavior<Lsp> {}

impl Contribution for Box<dyn LspBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Lsp` operation.
pub struct BasicLsp {
    /// Registered LSP handlers, keyed by method.
    pub rules: Vec<Box<dyn LspBehavior>>,
    /// Option schemas contributed for the LSP.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicLsp {
    /// Create a new `BasicLsp` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicLsp {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicLsp {
    fn id(&self) -> &str {
        "lsp.basic"
    }

    fn name(&self) -> &str {
        "Basic LSP"
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
        Ok(CapabilityResponse(serde_json::json!({"capabilities": {}})))
    }
}

impl RuleAcceptor for BasicLsp {
    type Rule = Box<dyn LspBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}
