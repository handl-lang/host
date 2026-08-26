use handl_ext_lexer::{Lexer, LexerBehavior, LexerRequest};
use handl_platform::{Behavior, PlatformError};

pub struct DocComments;

impl Behavior<Lexer> for DocComments {
    fn kind(&self) -> &str {
        "doc_comment"
    }

    fn apply(&self, _request: &mut LexerRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl LexerBehavior for DocComments {}
