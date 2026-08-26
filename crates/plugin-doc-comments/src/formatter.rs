use handl_ext_fmt::{Format, FormatBehavior, FormatRequest};
use handl_platform::{Behavior, PlatformError};

pub struct DocComments;

impl Behavior<Format> for DocComments {
    fn kind(&self) -> &str {
        "doc_comment"
    }

    fn apply(&self, _request: &mut FormatRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl FormatBehavior for DocComments {}
