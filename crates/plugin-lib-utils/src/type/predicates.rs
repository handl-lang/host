use handl_platform::{EvalContext, EvalError, Function};

use crate::shared::{
    as_kind, complement, err_result, matches, ok_result, project, require_arg_count,
    require_one_arg,
};

#[derive(Clone, Copy)]
pub struct TypeOf;
impl Function for TypeOf {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = require_one_arg(args)?;
        let type_name = if arg.is_null() {
            "null"
        } else if arg.is_boolean() {
            "bool"
        } else if arg.is_number() {
            "num"
        } else if arg.is_string() {
            "str"
        } else if arg.is_array() {
            "arr"
        } else if arg.is_object() {
            "obj"
        } else {
            "unknown"
        };
        Ok(serde_json::Value::from(type_name.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct TypeIs;
impl Function for TypeIs {
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
pub struct TypeMust;
impl Function for TypeMust {
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
            None => Ok(err_result(&format!("type.must: expected {kind}"))),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TypeMay;
impl Function for TypeMay {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(project(value, kind).unwrap_or_else(|| serde_json::Value::from(serde_json::Value::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct TypeNot;
impl Function for TypeNot {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 2)?;
        let value = &args[0];
        let kind = as_kind(&args[1])?;
        Ok(complement(value, kind)
            .unwrap_or_else(|| serde_json::Value::from(serde_json::Value::Null)))
    }
}

#[derive(Clone, Copy)]
pub struct TypeWhen;
impl Function for TypeWhen {
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
pub struct StringTag(&'static str);
impl Function for StringTag {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        require_arg_count(args, 0)?;
        Ok(serde_json::Value::from(self.0.to_string()))
    }
}

pub static STRING_TAG: StringTag = StringTag("type.string");
pub static STR_TAG: StringTag = StringTag("type.str");
pub static NUMBER_TAG: StringTag = StringTag("type.number");
pub static NUM_TAG: StringTag = StringTag("type.num");
pub static BOOL_TAG: StringTag = StringTag("type.bool");
pub static NULL_TAG: StringTag = StringTag("type.null");
pub static ARRAY_TAG: StringTag = StringTag("type.array");
pub static ARR_TAG: StringTag = StringTag("type.arr");
pub static OBJECT_TAG: StringTag = StringTag("type.object");
pub static OBJ_TAG: StringTag = StringTag("type.obj");
pub static ANY_TAG: StringTag = StringTag("type.any");
pub static UNKNOWN_TAG: StringTag = StringTag("type.unknown");
