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

use std::sync::{Arc, OnceLock};

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value as JsonValue;

use super::decode::Utf8Iterator;
use super::ident::{is_identifier, is_identifier_continue, is_identifier_start};

/// The handl source declaring the unicode `op` interface — the single source
/// of truth for the unicode API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const UNICODE_HNL: &str = include_str!("../../../handl-packages/unicode/unicode.hnl");

/// The op names declared in `unicode.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const UNICODE_OPS: &[&str] = &[
    "unicode.is_valid_identifier",
    "unicode.is_xid_start",
    "unicode.is_xid_continue",
    "unicode.code_points",
    "unicode.code_point_at",
    "unicode.decode",
];

/// Build the [`HandlModule`] for unicode.
///
/// All functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_unicode_module() -> HandlModule {
    let registry = Arc::new(super::unicode_op_registry());
    HandlModule::new("unicode", UNICODE_HNL, &[]).with_ops(UNICODE_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for unicode.
static UNICODE_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for unicode, initializing it on first call.
fn unicode_module() -> &'static HandlModule {
    // `with_loader_disabled`: `HandlModule::new`'s `analyze_source` would
    // otherwise run `loader.analyze` — a nested eval whose `call_host`
    // misses probe `modules.lookup`, which can re-enter this `OnceLock`
    // and deadlock. The bootstrap path is pure text transformation.
    UNICODE_MODULE.get_or_init(|| {
        handl_plugin_lib_handl::with_loader_disabled(build_unicode_module)
    })
}

/// The `unicode` module — Unicode primitives for handl expressions.
///
/// The interface is declared in `unicode.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct only delegates
/// [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct UnicodeModule;

impl Module for UnicodeModule {
    fn name(&self) -> &str {
        "unicode"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        unicode_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        unicode_module().functions()
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
