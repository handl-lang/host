use handl_platform::{EvalContext, EvalError, Function};

use super::helpers::{num, require_number, require_two_numbers, unary_fn};

unary_fn!(Sqrt, |x: f64| x.sqrt());
unary_fn!(Log, |x: f64| x.ln());
unary_fn!(Log10, |x: f64| x.log10());
unary_fn!(Exp, |x: f64| x.exp());
unary_fn!(Log2, |x: f64| x.log2());
unary_fn!(Cbrt, |x: f64| x.cbrt());

#[derive(Clone, Copy)]
pub struct Pow;
impl Function for Pow {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (base, exp) = require_two_numbers(args)?;
        Ok(num(base.powf(exp)))
    }
}
