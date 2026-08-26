pub mod diagnostics;
pub mod formatter;
pub mod lexer;
pub mod parser;

use handl_ext_fmt::{BasicFormatter, FormatBehavior};
use handl_ext_lexer::{BasicLexer, LexerBehavior};
use handl_ext_lint::{BasicLinter, LintBehavior};
use handl_ext_parser::{BasicParser, ParserBehavior};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

pub struct CommentsPlugin;

impl CommentsPlugin {
    pub fn new() -> Self {
        CommentsPlugin
    }
}

impl Default for CommentsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CommentsPlugin {
    fn id(&self) -> &str {
        "comments"
    }

    fn name(&self) -> &str {
        "handl-plugin-comments"
    }

    fn version(&self) -> &str {
        "0.1.0"
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

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let lexer: Box<dyn LexerBehavior> = Box::new(lexer::Comments);
        ctx.contribute::<BasicLexer>(lexer)?;

        let parser: Box<dyn ParserBehavior> = Box::new(parser::Comments);
        ctx.contribute::<BasicParser>(parser)?;

        let formatter: Box<dyn FormatBehavior> = Box::new(formatter::Comments);
        ctx.contribute::<BasicFormatter>(formatter)?;

        let diagnostics: Box<dyn LintBehavior> = Box::new(diagnostics::Comments);
        ctx.contribute::<BasicLinter>(diagnostics)?;

        Ok(())
    }
}
