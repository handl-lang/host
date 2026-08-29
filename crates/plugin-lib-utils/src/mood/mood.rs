use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;

use crate::shared::{
    as_kind, complement, err_result, matches, ok_result, project, require_arg_count, type_name,
};

#[derive(Clone, Copy)]
pub struct MoodInd;
impl Function<NonFunctionValue> for MoodInd {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        match project(value, kind) {
            Some(v) => Ok(v),
            None => Err(Error::PrimitiveError(format!(
                "mood.ind: expected {kind}, got {}",
                type_name(value)
            ))),
        }
    }
}

#[derive(Clone, Copy)]
pub struct MoodInt;
impl Function<NonFunctionValue> for MoodInt {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(NonFunctionValue::new(matches(value, kind)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodNorm;
impl Function<NonFunctionValue> for MoodNorm {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
impl Function<NonFunctionValue> for MoodPot {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(project(value, kind).unwrap_or_else(|| NonFunctionValue::new(JsonValue::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodNeg;
impl Function<NonFunctionValue> for MoodNeg {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(complement(value, kind).unwrap_or_else(|| NonFunctionValue::new(JsonValue::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct MoodCnd;
impl Function<NonFunctionValue> for MoodCnd {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        require_arg_count(args, 3)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        let default = &args[2];
        Ok(project(value, kind).unwrap_or_else(|| NonFunctionValue::new(default.inner().clone())))
    }
}

#[derive(Clone, Copy)]
pub struct MoodModule;
impl Module for MoodModule {
    fn name(&self) -> &str {
        "mood"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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
