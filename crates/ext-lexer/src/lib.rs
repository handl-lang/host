use handl_platform::{
    Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Lexer operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Lexer;

impl Operation for Lexer {
    type Request = LexerRequest;
}

/// Request passed to `Lexer` behaviors.
#[derive(Debug, Clone)]
pub struct LexerRequest {
    /// Source text to lex.
    pub source: String,
    /// Resolved lexer options.
    pub options: ResolvedOptions,
}

/// Behavior trait for lexing a specific `AstNode.kind`.
pub trait LexerBehavior: Behavior<Lexer> {}

impl Contribution for Box<dyn LexerBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Lexer` operation.
pub struct BasicLexer {
    /// Registered lexing rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn LexerBehavior>>,
    /// Option schemas contributed for the lexer.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicLexer {
    /// Create a new `BasicLexer` with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicLexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicLexer {
    fn id(&self) -> &str {
        "lexer.basic"
    }

    fn name(&self) -> &str {
        "Basic lexer"
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

impl RuleAcceptor for BasicLexer {
    type Rule = Box<dyn LexerBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}
