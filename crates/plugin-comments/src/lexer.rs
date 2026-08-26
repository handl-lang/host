use handl_ext_lexer::{Lexer, LexerBehavior, LexerRequest};
use handl_platform::{Behavior, PlatformError};

pub struct Comments;

impl Behavior<Lexer> for Comments {
    fn kind(&self) -> &str {
        "comment"
    }

    fn apply(&self, _request: &mut LexerRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl LexerBehavior for Comments {}
