use handl_ext_lint::{Lint, LintBehavior, LintRequest};
use handl_platform::{Behavior, PlatformError};

pub struct DocComments;

impl Behavior<Lint> for DocComments {
    fn kind(&self) -> &str {
        "doc_comment"
    }

    fn apply(&self, _request: &mut LintRequest) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl LintBehavior for DocComments {}
