use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Placeholder token type for `ParserRequest`.
#[derive(Debug, Clone)]
pub struct Token;

/// Parser operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Parser;

impl Operation for Parser {
    type Request = ParserRequest;
}

/// Request passed to `Parser` behaviors.
#[derive(Debug, Clone)]
pub struct ParserRequest {
    /// Tokens to parse.
    pub tokens: Vec<Token>,
    /// Resolved parser options.
    pub options: ResolvedOptions,
    /// Target AST node (placeholder; behaviors may set this).
    pub ast: AstNode,
}

/// Behavior trait for parsing a specific `AstNode.kind`.
pub trait ParserBehavior: Behavior<Parser> {}

impl Contribution for Box<dyn ParserBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Parser` operation.
pub struct BasicParser {
    /// Registered parsing rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn ParserBehavior>>,
    /// Option schemas contributed for the parser.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicParser {
    /// Create a new `BasicParser` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicParser {
    fn id(&self) -> &str {
        "parser.basic"
    }

    fn name(&self) -> &str {
        "Basic parser"
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

impl RuleAcceptor for BasicParser {
    type Rule = Box<dyn ParserBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}
