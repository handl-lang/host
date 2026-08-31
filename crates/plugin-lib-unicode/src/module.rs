//! The `unicode` handl runtime module.
//!
//! Exposes Unicode helpers callable from handl expressions:
//!
//! - `unicode.is_valid_identifier(string)` — `bool`
//! - `unicode.is_xid_start(string)` — `bool`
//! - `unicode.is_xid_continue(string)` — `bool`
//! - `unicode.code_points(string)` — array of numeric code points
//! - `unicode.code_point_at(string, index)` — number or `null`
//! - `unicode.decode(string)` — array of `[byte_offset, code_point]` pairs

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value as JsonValue;

use super::decode::Utf8Iterator;
use super::ident::{is_identifier, is_identifier_continue, is_identifier_start};

/// The `unicode` module — Unicode primitives for handl expressions.
#[derive(Clone, Copy)]
pub struct UnicodeModule;

impl Module for UnicodeModule {
    fn name(&self) -> &str {
        "unicode"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "is_valid_identifier" => Some(&IsValidIdentifier),
            "is_xid_start" => Some(&IsXidStart),
            "is_xid_continue" => Some(&IsXidContinue),
            "code_points" => Some(&CodePoints),
            "code_point_at" => Some(&CodePointAt),
            "decode" => Some(&Decode),
            _ => None,
        }
    }
}

fn require_one_string(args: &[serde_json::Value]) -> Result<String, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1,
        args.len(),
    )))?;
    arg.as_str()
        .map(|s| s.to_string())
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn require_string_and_index(args: &[serde_json::Value]) -> Result<(String, usize), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let s = args[0]
        .as_str()
        .map(|s| s.to_string())
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
    let n = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number", "non-number",
    )))?;
    if n < 0.0 || n.fract() != 0.0 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "non-negative integer", "invalid index",
        )));
    }
    Ok((s, n as usize))
}

fn bool_(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

fn num(value: f64) -> serde_json::Value {
    serde_json::Value::from(value)
}

fn null() -> serde_json::Value {
    serde_json::Value::from(JsonValue::Null)
}

#[derive(Clone, Copy)]
pub struct IsValidIdentifier;
impl Function for IsValidIdentifier {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        Ok(bool_(is_identifier(&s)))
    }
}

#[derive(Clone, Copy)]
pub struct IsXidStart;
impl Function for IsXidStart {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let first = s.chars().next();
        Ok(bool_(first.map_or(false, is_identifier_start)))
    }
}

#[derive(Clone, Copy)]
pub struct IsXidContinue;
impl Function for IsXidContinue {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let first = s.chars().next();
        Ok(bool_(first.map_or(false, is_identifier_continue)))
    }
}

#[derive(Clone, Copy)]
pub struct CodePoints;
impl Function for CodePoints {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let points: Vec<JsonValue> = s
            .chars()
            .map(|ch| JsonValue::Number(serde_json::Number::from(ch as u32)))
            .collect();
        Ok(serde_json::Value::from(JsonValue::Array(points)))
    }
}

#[derive(Clone, Copy)]
pub struct CodePointAt;
impl Function for CodePointAt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (s, index) = require_string_and_index(args)?;
        match s.chars().nth(index) {
            Some(ch) => Ok(num(ch as u32 as f64)),
            None => Ok(null()),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Decode;
impl Function for Decode {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let mut pairs = Vec::new();
        for item in Utf8Iterator::new(s.as_bytes()) {
            let (offset, ch) =
                item.map_err(|_| EvalError::Other("invalid UTF-8 in string".to_string()))?;
            let pair = JsonValue::Array(vec![
                JsonValue::Number(serde_json::Number::from(offset)),
                JsonValue::Number(serde_json::Number::from(ch as u32)),
            ]);
            pairs.push(pair);
        }
        Ok(serde_json::Value::from(JsonValue::Array(pairs)))
    }
}
