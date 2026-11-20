//! Bridge between the kernel op ABI ([`handl_ops`]) and the host eval model.
//!
//! [`EvalContext`] implements [`handl_ops::OpContext`] so kernel op crates
//! (`handl-lib-*`, `lib-*`) can run under the host evaluator without
//! depending on `handl-platform`. Error conversions run both directions:
//! host [`EvalError`] → [`handl_ops::OpError`] when an op observes a host
//! failure, and back again when a host `Function` dispatches into a kernel
//! [`handl_ops::Op`].
//!
//! [`OpError::Yield`](handl_ops::OpError::Yield) maps back to
//! [`EvalError::Other`]: a yield's continuation context is host state that
//! cannot cross the ABI boundary. Ops only observe yields (e.g. `error.try`
//! reporting one); propagating one upward through a kernel op degrades to a
//! plain error — acceptable because ops never *produce* yields.

use handl_ops::{OpContext, OpError};
use serde_json::Value;

use crate::eval::{EvalContext, EvalError};

impl OpContext for EvalContext {
    fn eval_expr(&mut self, expr: &str, vars: &[(String, Value)]) -> Result<Value, OpError> {
        EvalContext::eval_expr(self, expr, vars).map_err(OpError::from)
    }

    fn try_call_value(&mut self, callee: &Value, args: &[Value]) -> Option<Result<Value, OpError>> {
        EvalContext::try_call_value(self, callee, args).map(|r| r.map_err(OpError::from))
    }
}

impl From<EvalError> for OpError {
    fn from(e: EvalError) -> Self {
        match e {
            EvalError::TypeMismatch(s) => OpError::TypeMismatch(s),
            EvalError::UnknownFunction(s) => OpError::UnknownFunction(s),
            EvalError::UnknownVariable(s) => OpError::UnknownVariable(s),
            EvalError::ResourceLimit(s) => OpError::ResourceLimit(s),
            EvalError::Yield { .. } => OpError::Yield,
            EvalError::Other(s) => OpError::Other(s),
        }
    }
}

impl From<OpError> for EvalError {
    fn from(e: OpError) -> Self {
        match e {
            OpError::TypeMismatch(s) => EvalError::TypeMismatch(s),
            OpError::UnknownFunction(s) => EvalError::UnknownFunction(s),
            OpError::UnknownVariable(s) => EvalError::UnknownVariable(s),
            OpError::ResourceLimit(s) => EvalError::ResourceLimit(s),
            OpError::Yield => EvalError::Other("evaluation yielded".to_string()),
            OpError::Other(s) => EvalError::Other(s),
        }
    }
}
