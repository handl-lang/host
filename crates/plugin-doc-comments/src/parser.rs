use handl_ext_parser::{Parser, ParserBehavior, ParserRequest};
use handl_platform::{Behavior, PlatformError};

pub struct DocComments;

impl Behavior<Parser> for DocComments {
    fn kind(&self) -> &str {
        "doc_comment"
    }

    fn apply(&self, _request: &mut ParserRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl ParserBehavior for DocComments {}
