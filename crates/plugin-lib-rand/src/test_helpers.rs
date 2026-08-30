use handl_platform::Module;
use handl_platform::{EvalContext, EvalError};

use super::module::RandModule;

pub fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = RandModule.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

pub fn call_err(name: &str, args: &[serde_json::Value]) -> Result<serde_json::Value, EvalError> {
    let func = RandModule.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

pub fn n(v: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(v))
}

pub fn arr(values: &[serde_json::Value]) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Array(values.to_vec()))
}

pub fn extract_num(v: &serde_json::Value) -> f64 {
    v.as_f64().expect("expected number")
}

pub fn extract_str(v: &serde_json::Value) -> String {
    v.as_str().expect("expected string").to_string()
}

pub fn extract_array(v: &serde_json::Value) -> Vec<serde_json::Value> {
    v.as_array().expect("expected array").to_vec()
}
