use handl_ext_lint::{Lint, LintBehavior, LintRequest};
use handl_platform::{Behavior, PlatformError};

pub struct Comments;

impl Behavior<Lint> for Comments {
    fn kind(&self) -> &str {
        "comment"
    }

    fn apply(&self, _request: &mut LintRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl LintBehavior for Comments {}
