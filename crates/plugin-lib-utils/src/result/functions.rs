use handl_platform::{EvalContext, EvalError, Function};
use serde_json::{Value as JsonValue, json};

fn as_result_tag(value: &JsonValue) -> Option<&str> {
    value.get("_tag")?.as_str()
}

#[derive(Clone, Copy)]
pub struct Ok;
impl Function for Ok {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        std::result::Result::Ok(serde_json::Value::from(json!({
            "_tag": "Ok",
            "value": arg.clone(),
        })))
    }
}

#[derive(Clone, Copy)]
pub struct Err;
impl Function for Err {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        std::result::Result::Ok(serde_json::Value::from(json!({
            "_tag": "Err",
            "error": arg.clone(),
        })))
    }
}

#[derive(Clone, Copy)]
pub struct IsOk;
impl Function for IsOk {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        let is_ok = as_result_tag(arg) == Some("Ok");
        std::result::Result::Ok(serde_json::Value::from(json!(is_ok)))
    }
}

#[derive(Clone, Copy)]
pub struct IsErr;
impl Function for IsErr {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        let is_err = as_result_tag(arg) == Some("Err");
        std::result::Result::Ok(serde_json::Value::from(json!(is_err)))
    }
}
