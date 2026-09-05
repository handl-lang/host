use handl_platform::{EvalError, ExprEvaluator};
use serde_json::Value;

/// An [`ExprEvaluator`] that always returns `true`.
pub struct TrueEval;

impl ExprEvaluator for TrueEval {
    fn eval_expr(&self, _expr: &str, _vars: &[(String, Value)]) -> Result<Value, EvalError> {
        Ok(Value::Bool(true))
    }
}

/// An [`ExprEvaluator`] that always fails.
pub struct ErrorEval;

impl ExprEvaluator for ErrorEval {
    fn eval_expr(&self, _expr: &str, _vars: &[(String, Value)]) -> Result<Value, EvalError> {
        Err(EvalError::Other("eval error".to_string()))
    }
}

/// An [`ExprEvaluator`] that returns the value bound to the first variable.
pub struct VarEval;

impl ExprEvaluator for VarEval {
    fn eval_expr(&self, _expr: &str, vars: &[(String, Value)]) -> Result<Value, EvalError> {
        Ok(vars
            .first()
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| Value::Null))
    }
}
