use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

pub struct FmtPlugin {
    capabilities: Vec<CapabilityId>,
}

impl FmtPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt")],
        }
    }
}

impl Default for FmtPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FmtPlugin {
    fn name(&self) -> &str {
        "handl-ext-fmt"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "fmt",
                "description": "Format a Handl module"
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
        Ok(CapabilityResponse(serde_json::json!({"formatted": true})))
    }
}

/// Format operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Format;

impl Operation for Format {
    type Request = FormatRequest;
}

/// Request passed to `Format` behaviors.
#[derive(Debug, Clone)]
pub struct FormatRequest {
    /// The AST node to format.
    pub ast: AstNode,
    /// Resolved formatter options.
    pub options: ResolvedOptions,
}

/// Behavior trait for formatting a specific `AstNode.kind`.
pub trait FormatBehavior: Behavior<Format> {}

impl Contribution for Box<dyn FormatBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Format` operation.
pub struct BasicFormatter {
    /// Registered formatting rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn FormatBehavior>>,
    /// Option schemas contributed for the formatter.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicFormatter {
    /// Create a new `BasicFormatter` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Plugin for BasicFormatter {
    fn id(&self) -> &str {
        "fmt.basic"
    }

    fn name(&self) -> &str {
        "Basic formatter"
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

impl RuleAcceptor for BasicFormatter {
    type Rule = Box<dyn FormatBehavior>;

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
        let plugin = FmtPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("fmt")).unwrap();
        assert_eq!(meta["command"], "fmt");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_fmted_flag() {
        let plugin = FmtPlugin::new();
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
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"formatted": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = FmtPlugin::new();
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
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
