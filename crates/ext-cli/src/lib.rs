use handl_platform::{
    Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// CLI operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Cli;

impl Operation for Cli {
    type Request = CliRequest;
}

/// Request passed to `Cli` behaviors.
#[derive(Debug, Clone)]
pub struct CliRequest {
    /// CLI command name.
    pub command: String,
    /// CLI command arguments.
    pub args: Vec<String>,
    /// Resolved CLI options.
    pub options: ResolvedOptions,
}

/// Behavior trait for handling a specific CLI command.
pub trait CliBehavior: Behavior<Cli> {}

impl Contribution for Box<dyn CliBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Cli` operation.
pub struct BasicCli {
    /// Registered CLI behaviors, keyed by command.
    pub rules: Vec<Box<dyn CliBehavior>>,
    /// Option schemas contributed for the CLI.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicCli {
    /// Create a new `BasicCli` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicCli {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicCli {
    fn id(&self) -> &str {
        "cli.basic"
    }

    fn name(&self) -> &str {
        "Basic CLI"
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
        Ok(CapabilityResponse(serde_json::json!({"commands": []})))
    }
}

impl RuleAcceptor for BasicCli {
    type Rule = Box<dyn CliBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}
