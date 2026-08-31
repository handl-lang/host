use super::common::{b, require_one_string};
use handl_platform::{EvalContext, EvalError, Function};

#[derive(Clone, Copy)]
pub struct IsValid;
impl Function for IsValid {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let valid = super::common::parse_iso(input).is_ok();
        Ok(b(valid))
    }
}
