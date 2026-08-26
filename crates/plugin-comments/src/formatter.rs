use handl_ext_fmt::{Format, FormatBehavior, FormatRequest};
use handl_platform::{Behavior, PlatformError};

pub struct Comments;

impl Behavior<Format> for Comments {
    fn kind(&self) -> &str {
        "comment"
    }

    fn apply(&self, _request: &mut FormatRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl FormatBehavior for Comments {}
