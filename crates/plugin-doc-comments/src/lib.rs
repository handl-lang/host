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

pub struct DocCommentsPlugin;

impl DocCommentsPlugin {
    pub fn new() -> Self {
        DocCommentsPlugin
    }
}

impl Default for DocCommentsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for DocCommentsPlugin {
    fn id(&self) -> &str {
        "doc-comments"
    }

    fn name(&self) -> &str {
        "handl-plugin-doc-comments"
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
        let lexer: Box<dyn LexerBehavior> = Box::new(lexer::DocComments);
        ctx.contribute::<BasicLexer>(lexer)?;

        let parser: Box<dyn ParserBehavior> = Box::new(parser::DocComments);
        ctx.contribute::<BasicParser>(parser)?;

        let formatter: Box<dyn FormatBehavior> = Box::new(formatter::DocComments);
        ctx.contribute::<BasicFormatter>(formatter)?;

        let diagnostics: Box<dyn LintBehavior> = Box::new(diagnostics::DocComments);
        ctx.contribute::<BasicLinter>(diagnostics)?;

        Ok(())
    }
}
