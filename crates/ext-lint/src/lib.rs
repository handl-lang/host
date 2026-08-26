use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

pub struct LintPlugin {
    capabilities: Vec<CapabilityId>,
}

impl LintPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("lint")],
        }
    }
}

impl Default for LintPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for LintPlugin {
    fn name(&self) -> &str {
        "handl-ext-lint"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "lint",
                "description": "Lint a Handl module"
            }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        Ok(CapabilityResponse(serde_json::json!({"linted": true})))
    }
}

/// Lint operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Lint;

impl Operation for Lint {
    type Request = LintRequest;
}

/// Request passed to `Lint` behaviors.
#[derive(Debug, Clone)]
pub struct LintRequest {
    /// The AST node to lint.
    pub ast: AstNode,
    /// Resolved linter options.
    pub options: ResolvedOptions,
}

/// Behavior trait for linting a specific `AstNode.kind`.
pub trait LintBehavior: Behavior<Lint> {}

impl Contribution for Box<dyn LintBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Lint` operation.
pub struct BasicLinter {
    /// Registered linting rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn LintBehavior>>,
    /// Option schemas contributed for the linter.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicLinter {
    /// Create a new `BasicLinter` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Plugin for BasicLinter {
    fn id(&self) -> &str {
        "lint.basic"
    }

    fn name(&self) -> &str {
        "Basic linter"
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

impl RuleAcceptor for BasicLinter {
    type Rule = Box<dyn LintBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = LintPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("lint")).unwrap();
        assert_eq!(meta["command"], "lint");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_linted_flag() {
        let plugin = LintPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"linted": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = LintPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
