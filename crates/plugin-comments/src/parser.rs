use handl_ext_parser::{Parser, ParserBehavior, ParserRequest};
use handl_platform::{Behavior, PlatformError};

pub struct Comments;

impl Behavior<Parser> for Comments {
    fn kind(&self) -> &str {
        "comment"
    }

    fn apply(&self, _request: &mut ParserRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl ParserBehavior for Comments {}
