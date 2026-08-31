use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value as JsonValue;

use crate::shared::{
    as_kind, complement, err_result, matches, ok_result, project, require_arg_count, type_name,
};

#[derive(Clone, Copy)]
pub struct MoodInd;
impl Function for MoodInd {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        match project(value, kind) {
            Some(v) => Ok(v),
            None => Err(EvalError::Other(format!(
                "mood.ind: expected {kind}, got {}",
                type_name(value)
            ))),
        }
    }
}

#[derive(Clone, Copy)]
pub struct MoodInt;
impl Function for MoodInt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(serde_json::Value::from(matches(value, kind)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodNorm;
impl Function for MoodNorm {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        match project(value, kind) {
            Some(v) => Ok(ok_result(v)),
            None => Ok(err_result(&format!("mood.norm: expected {kind}"))),
        }
    }
}

#[derive(Clone, Copy)]
pub struct MoodPot;
impl Function for MoodPot {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(project(value, kind).unwrap_or_else(|| serde_json::Value::from(JsonValue::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodNeg;
impl Function for MoodNeg {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(complement(value, kind).unwrap_or_else(|| serde_json::Value::from(JsonValue::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodCnd;
impl Function for MoodCnd {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 3)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        let default = &args[2];
        Ok(project(value, kind).unwrap_or_else(|| serde_json::Value::from(default.clone())))
    }
}

#[derive(Clone, Copy)]
pub struct MoodModule;
impl Module for MoodModule {
    fn name(&self) -> &str {
        "mood"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "ind" => Some(&MoodInd),
            "int" => Some(&MoodInt),
            "norm" => Some(&MoodNorm),
            "pot" => Some(&MoodPot),
            "neg" => Some(&MoodNeg),
            "cnd" => Some(&MoodCnd),
            _ => None,
        }
    }
}
