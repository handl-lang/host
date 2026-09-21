//! Shared runtime value layout and helper ABI (ADR-0114 Phase 2).
//!
//! `ValueAbi` is the in-memory abstraction of a Handl runtime value. It mirrors
//! the tagged `serde_json::Value` layout used by the tree-walk, bytecode, and
//! cranelift engines so that `plugin-compile-js` and `plugin-compile-llvm` can
//! agree on a single serialization and calling convention.
//!
//! # Number representation
//!
//! Numbers are stored as [`serde_json::Number`]. This keeps the primary
//! representation JSON-compatible and avoids losing the distinction between
//! integer and floating-point literals at the ABI boundary. Engines that need
//! machine `i64`/`f64` values can lower them from this type at compile time.
//!
//! # Helper ABI
//!
//! The `helper_v2_*` functions declared in the cranelift runtime are represented
//! here as `unsafe extern "C" fn` function-pointer types. They use the
//! `*mut Value` and `*mut EvalCtx` aliases below. `EvalCtx` is deliberately an
//! opaque `c_void` alias so this crate does not need to depend on the helper
//! implementation; the actual runtime uses `crates/plugin-engine-exec-cranelift`.

use std::collections::HashMap;
use std::ffi::c_void;

use serde_json::{Number, Value as JsonValue};

use crate::CompileError;

/// Discriminant tag for a [`ValueAbi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// The null value.
    Null,
    /// Boolean.
    Bool,
    /// Numeric value.
    Number,
    /// Unicode string.
    String,
    /// Array of values.
    Array,
    /// String-keyed record.
    Object,
    /// Raw byte buffer.
    Bytes,
    /// Product/pair value.
    Pair,
    /// Either left injection.
    EitherLeft,
    /// Either right injection.
    EitherRight,
    /// Function closure.
    Arrow,
    /// Opaque runtime handle.
    Handle,
    /// Host resource pointer (debug only).
    Resource,
}

impl Tag {
    /// Return the canonical string name for this tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            Tag::Null => "null",
            Tag::Bool => "bool",
            Tag::Number => "number",
            Tag::String => "string",
            Tag::Array => "array",
            Tag::Object => "object",
            Tag::Bytes => "bytes",
            Tag::Pair => "pair",
            Tag::EitherLeft => "either-left",
            Tag::EitherRight => "either-right",
            Tag::Arrow => "arrow",
            Tag::Handle => "handle",
            Tag::Resource => "resource",
        }
    }
}

/// Alias for `ValueAbi` so helper signatures can be written as `*mut Value`.
pub type Value = ValueAbi;

/// Alias for `c_void` so helper signatures can be written as `*mut EvalCtx`.
pub type EvalCtx = c_void;

/// Runtime value layout used by the compiler targets.
///
/// This is the shared in-memory shape produced and consumed by the
/// `helper_v2_*` runtime functions. It is intentionally close to
/// `serde_json::Value` so that host interop and debug serialization
/// are a single round-trip.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueAbi {
    /// The null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// JSON number (integer or floating-point).
    Number(Number),
    /// Unicode string.
    String(String),
    /// Array of values.
    Array(Vec<ValueAbi>),
    /// String-keyed record.
    Object(HashMap<String, ValueAbi>),
    /// Raw byte buffer.
    Bytes(Vec<u8>),
    /// Product/pair value.
    Pair(Box<ValueAbi>, Box<ValueAbi>),
    /// Either left injection.
    EitherLeft(Box<ValueAbi>),
    /// Either right injection.
    EitherRight(Box<ValueAbi>),
    /// Function closure.
    Arrow(Arrow),
    /// Opaque runtime handle (debug only, serialized as `__resource__`).
    Handle(u64),
    /// Host resource pointer (debug only).
    Resource {
        /// Opaque host address.
        ptr: u64,
    },
}

/// A function closure (arrow) — self-contained serializable data
/// (ADR-0106): no captured environment, no home frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrow {
    /// Parameter names.
    pub params: Vec<String>,
    /// The body as a `{"__ast__": <Ast>}` value — the substituted,
    /// standalone arena the arrow evaluates against.
    pub body_def: Option<Box<ValueAbi>>,
    /// Residual explicit bindings (rec-region binders, non-serializable
    /// captures) referenced by the body.
    pub binds: HashMap<String, ValueAbi>,
    /// Optional rest parameter name.
    pub rest: Option<String>,
    /// Optional array-pattern flag.
    pub array_pattern: Option<bool>,
    /// Handlers carried by deep-handler attach (ADR-0076).
    pub handlers: Vec<ValueAbi>,
}

impl ValueAbi {
    /// Return whether this value has the given tag.
    pub fn is_tag(&self, tag: Tag) -> bool {
        self.tag() == tag
    }

    /// Require this value to have the given tag.
    pub fn expect_tag(&self, tag: Tag) -> Result<&Self, CompileError> {
        if self.is_tag(tag) {
            Ok(self)
        } else {
            Err(CompileError::InvalidInput(format!(
                "expected tag {}, found {}",
                tag.as_str(),
                self.tag().as_str()
            )))
        }
    }

    /// Recursively validate that the payload matches this value's tag.
    pub fn check(&self) -> Result<(), CompileError> {
        match self {
            ValueAbi::Null
            | ValueAbi::Bool(_)
            | ValueAbi::Number(_)
            | ValueAbi::String(_)
            | ValueAbi::Bytes(_)
            | ValueAbi::Handle(_) => Ok(()),
            ValueAbi::Array(items) => items.iter().try_for_each(|v| v.check()),
            ValueAbi::Object(map) => {
                for (k, v) in map.iter() {
                    if matches!(
                        k.as_str(),
                        "__pair__" | "__either__" | "__arrow__" | "__bytes__" | "__resource__"
                    ) {
                        return Err(CompileError::InvalidInput(format!(
                            "reserved tag key '{k}' used in generic object"
                        )));
                    }
                    v.check()?;
                }
                Ok(())
            }
            ValueAbi::Pair(a, b) => {
                a.check()?;
                b.check()
            }
            ValueAbi::EitherLeft(v) | ValueAbi::EitherRight(v) => v.check(),
            ValueAbi::Arrow(arrow) => {
                if let Some(def) = &arrow.body_def {
                    def.check()?;
                }
                for value in arrow.binds.values() {
                    value.check()?;
                }
                for value in &arrow.handlers {
                    value.check()?;
                }
                Ok(())
            }
            ValueAbi::Resource { .. } => Ok(()),
        }
    }
    /// Return the tag of this value.
    pub fn tag(&self) -> Tag {
        match self {
            ValueAbi::Null => Tag::Null,
            ValueAbi::Bool(_) => Tag::Bool,
            ValueAbi::Number(_) => Tag::Number,
            ValueAbi::String(_) => Tag::String,
            ValueAbi::Array(_) => Tag::Array,
            ValueAbi::Object(_) => Tag::Object,
            ValueAbi::Bytes(_) => Tag::Bytes,
            ValueAbi::Pair(_, _) => Tag::Pair,
            ValueAbi::EitherLeft(_) => Tag::EitherLeft,
            ValueAbi::EitherRight(_) => Tag::EitherRight,
            ValueAbi::Arrow(_) => Tag::Arrow,
            ValueAbi::Handle(_) => Tag::Handle,
            ValueAbi::Resource { .. } => Tag::Resource,
        }
    }

    /// Convert this runtime value to its canonical JSON representation.
    pub fn to_json(&self) -> JsonValue {
        match self {
            ValueAbi::Null => JsonValue::Null,
            ValueAbi::Bool(b) => JsonValue::Bool(*b),
            ValueAbi::Number(n) => JsonValue::Number(n.clone()),
            ValueAbi::String(s) => JsonValue::String(s.clone()),
            ValueAbi::Array(items) => {
                JsonValue::Array(items.iter().map(ValueAbi::to_json).collect())
            }
            ValueAbi::Object(fields) => JsonValue::Object(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect(),
            ),
            ValueAbi::Bytes(buf) => {
                let arr: Vec<JsonValue> = buf
                    .iter()
                    .map(|b| JsonValue::Number(Number::from(*b)))
                    .collect();
                let mut map = serde_json::Map::new();
                map.insert("__bytes__".to_string(), JsonValue::Array(arr));
                JsonValue::Object(map)
            }
            ValueAbi::Pair(a, b) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "__pair__".to_string(),
                    JsonValue::Array(vec![a.to_json(), b.to_json()]),
                );
                JsonValue::Object(map)
            }
            ValueAbi::EitherLeft(v) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "__either__".to_string(),
                    JsonValue::String("left".to_string()),
                );
                map.insert("value".to_string(), v.to_json());
                JsonValue::Object(map)
            }
            ValueAbi::EitherRight(v) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "__either__".to_string(),
                    JsonValue::String("right".to_string()),
                );
                map.insert("value".to_string(), v.to_json());
                JsonValue::Object(map)
            }
            ValueAbi::Arrow(arrow) => {
                let mut map = serde_json::Map::new();
                map.insert("__arrow__".to_string(), JsonValue::Bool(true));
                map.insert(
                    "params".to_string(),
                    JsonValue::Array(
                        arrow
                            .params
                            .iter()
                            .map(|s| JsonValue::String(s.clone()))
                            .collect(),
                    ),
                );
                if let Some(def) = &arrow.body_def {
                    map.insert("body_def".to_string(), def.to_json());
                }
                if !arrow.binds.is_empty() {
                    map.insert(
                        "binds".to_string(),
                        JsonValue::Object(
                            arrow
                                .binds
                                .iter()
                                .map(|(k, v)| (k.clone(), v.to_json()))
                                .collect(),
                        ),
                    );
                }
                if !arrow.handlers.is_empty() {
                    map.insert(
                        "handlers".to_string(),
                        JsonValue::Array(arrow.handlers.iter().map(ValueAbi::to_json).collect()),
                    );
                }
                if let Some(rest) = &arrow.rest {
                    map.insert("rest".to_string(), JsonValue::String(rest.clone()));
                }
                if let Some(array_pattern) = arrow.array_pattern {
                    map.insert("array_pattern".to_string(), JsonValue::Bool(array_pattern));
                }
                JsonValue::Object(map)
            }
            ValueAbi::Handle(ptr) | ValueAbi::Resource { ptr } => {
                let mut inner = serde_json::Map::new();
                inner.insert("ptr".to_string(), JsonValue::Number(Number::from(*ptr)));
                let mut map = serde_json::Map::new();
                map.insert("__resource__".to_string(), JsonValue::Object(inner));
                JsonValue::Object(map)
            }
        }
    }

    /// Parse a runtime value from its canonical JSON representation.
    pub fn from_json(value: &JsonValue) -> Result<ValueAbi, CompileError> {
        match value {
            JsonValue::Null => Ok(ValueAbi::Null),
            JsonValue::Bool(b) => Ok(ValueAbi::Bool(*b)),
            JsonValue::Number(n) => Ok(ValueAbi::Number(n.clone())),
            JsonValue::String(s) => Ok(ValueAbi::String(s.clone())),
            JsonValue::Array(items) => Ok(ValueAbi::Array(
                items
                    .iter()
                    .map(ValueAbi::from_json)
                    .collect::<Result<_, _>>()?,
            )),
            JsonValue::Object(obj) => Self::from_object(obj),
        }
    }

    fn from_object(obj: &serde_json::Map<String, JsonValue>) -> Result<ValueAbi, CompileError> {
        if let Some(pair) = obj.get("__pair__") {
            let arr = pair.as_array().ok_or_else(|| {
                CompileError::InvalidInput("__pair__ must be an array".to_string())
            })?;
            if arr.len() != 2 {
                return Err(CompileError::InvalidInput(
                    "__pair__ array must have exactly two elements".to_string(),
                ));
            }
            return Ok(ValueAbi::Pair(
                Box::new(ValueAbi::from_json(&arr[0])?),
                Box::new(ValueAbi::from_json(&arr[1])?),
            ));
        }

        if let Some(side) = obj.get("__either__") {
            let side = side.as_str().ok_or_else(|| {
                CompileError::InvalidInput("__either__ must be a string".to_string())
            })?;
            let val = obj.get("value").ok_or_else(|| {
                CompileError::InvalidInput("__either__ missing 'value'".to_string())
            })?;
            return match side {
                "left" => Ok(ValueAbi::EitherLeft(Box::new(ValueAbi::from_json(val)?))),
                "right" => Ok(ValueAbi::EitherRight(Box::new(ValueAbi::from_json(val)?))),
                _ => Err(CompileError::InvalidInput(format!(
                    "unknown __either__ side: {side}"
                ))),
            };
        }

        if let Some(flag) = obj.get("__arrow__") {
            if !flag.as_bool().unwrap_or(false) {
                return Err(CompileError::InvalidInput(
                    "__arrow__ must be true".to_string(),
                ));
            }
            return Self::from_arrow(obj);
        }

        if let Some(bytes) = obj.get("__bytes__") {
            let arr = bytes.as_array().ok_or_else(|| {
                CompileError::InvalidInput("__bytes__ must be an array".to_string())
            })?;
            let mut buf = Vec::with_capacity(arr.len());
            for v in arr {
                let n = v.as_u64().ok_or_else(|| {
                    CompileError::InvalidInput("bytes must be numbers".to_string())
                })?;
                if n > u8::MAX as u64 {
                    return Err(CompileError::InvalidInput(
                        "byte values must be in the range 0..=255".to_string(),
                    ));
                }
                buf.push(n as u8);
            }
            return Ok(ValueAbi::Bytes(buf));
        }

        if let Some(inner) = obj.get("__resource__") {
            let inner = inner.as_object().ok_or_else(|| {
                CompileError::InvalidInput("__resource__ must be an object".to_string())
            })?;
            let ptr = inner.get("ptr").and_then(|v| v.as_u64()).ok_or_else(|| {
                CompileError::InvalidInput("__resource__ needs 'ptr'".to_string())
            })?;
            return Ok(ValueAbi::Resource { ptr });
        }

        let mut map = HashMap::new();
        for (k, v) in obj {
            map.insert(k.clone(), ValueAbi::from_json(v)?);
        }
        Ok(ValueAbi::Object(map))
    }

    fn from_arrow(obj: &serde_json::Map<String, JsonValue>) -> Result<ValueAbi, CompileError> {
        let params_json = obj
            .get("params")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CompileError::InvalidInput("arrow needs 'params' array".to_string()))?;
        let mut params = Vec::with_capacity(params_json.len());
        for p in params_json {
            let s = p.as_str().ok_or_else(|| {
                CompileError::InvalidInput("arrow params must be strings".to_string())
            })?;
            params.push(s.to_string());
        }

        let body_def = match obj.get("body_def") {
            Some(v) => Some(Box::new(ValueAbi::from_json(v)?)),
            None => None,
        };

        let mut binds = HashMap::new();
        if let Some(binds_json) = obj.get("binds").and_then(|v| v.as_object()) {
            for (k, v) in binds_json {
                binds.insert(k.clone(), ValueAbi::from_json(v)?);
            }
        }

        let mut handlers = Vec::new();
        if let Some(handlers_json) = obj.get("handlers").and_then(|v| v.as_array()) {
            for v in handlers_json {
                handlers.push(ValueAbi::from_json(v)?);
            }
        }

        let rest = if let Some(v) = obj.get("rest") {
            Some(
                v.as_str()
                    .ok_or_else(|| {
                        CompileError::InvalidInput("arrow 'rest' must be a string".to_string())
                    })?
                    .to_string(),
            )
        } else {
            None
        };

        let array_pattern = if let Some(v) = obj.get("array_pattern") {
            Some(v.as_bool().ok_or_else(|| {
                CompileError::InvalidInput("arrow 'array_pattern' must be a bool".to_string())
            })?)
        } else {
            None
        };

        Ok(ValueAbi::Arrow(Arrow {
            params,
            body_def,
            binds,
            rest,
            array_pattern,
            handlers,
        }))
    }
}

/// Helper name constants and `unsafe extern "C" fn` signatures.
///
/// This registry matches the cranelift runtime in
/// `crates/plugin-engine-exec-cranelift/src/lib.rs`. The function pointer
/// types use the `Value` and `EvalCtx` aliases from this module so that the
/// ABI is self-contained.
pub mod helpers {
    use super::{EvalCtx, Value};

    /// Name of the `helper_v2_clone` runtime function.
    pub const HELPER_V2_CLONE: &str = "helper_v2_clone";
    /// `helper_v2_clone` function pointer type.
    pub type CloneFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_call_ffi_raw` runtime function.
    pub const HELPER_V2_CALL_FFI_RAW: &str = "helper_v2_call_ffi_raw";
    /// `helper_v2_call_ffi_raw` function pointer type.
    pub type CallFfiRawFn =
        unsafe extern "C" fn(*mut EvalCtx, usize, *const *mut Value, usize) -> *mut Value;

    /// Name of the `helper_v2_push_literal` runtime function.
    pub const HELPER_V2_PUSH_LITERAL: &str = "helper_v2_push_literal";
    /// `helper_v2_push_literal` function pointer type.
    pub type PushLiteralFn = unsafe extern "C" fn(*mut EvalCtx, usize) -> *mut Value;

    /// Name of the `helper_v2_load_var` runtime function.
    pub const HELPER_V2_LOAD_VAR: &str = "helper_v2_load_var";
    /// `helper_v2_load_var` function pointer type.
    pub type LoadVarFn = unsafe extern "C" fn(*mut EvalCtx, usize) -> *mut Value;

    /// Name of the `helper_v2_load_payload_var` runtime function.
    pub const HELPER_V2_LOAD_PAYLOAD_VAR: &str = "helper_v2_load_payload_var";
    /// `helper_v2_load_payload_var` function pointer type.
    pub type LoadPayloadVarFn = unsafe extern "C" fn(*mut EvalCtx) -> *mut Value;

    /// Name of the `helper_v2_load_payload` runtime function.
    pub const HELPER_V2_LOAD_PAYLOAD: &str = "helper_v2_load_payload";
    /// `helper_v2_load_payload` function pointer type.
    pub type LoadPayloadFn = unsafe extern "C" fn(*mut EvalCtx) -> *mut Value;

    /// Name of the `helper_v2_eval_child` runtime function.
    pub const HELPER_V2_EVAL_CHILD: &str = "helper_v2_eval_child";
    /// `helper_v2_eval_child` function pointer type.
    pub type EvalChildFn = unsafe extern "C" fn(*mut EvalCtx, usize, *mut Value, i64) -> *mut Value;

    /// Name of the `helper_v2_extend_env` runtime function.
    pub const HELPER_V2_EXTEND_ENV: &str = "helper_v2_extend_env";
    /// `helper_v2_extend_env` function pointer type.
    pub type ExtendEnvFn = unsafe extern "C" fn(*mut EvalCtx, usize, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_extend_env_payload` runtime function.
    pub const HELPER_V2_EXTEND_ENV_PAYLOAD: &str = "helper_v2_extend_env_payload";
    /// `helper_v2_extend_env_payload` function pointer type.
    pub type ExtendEnvPayloadFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_pop_scope` runtime function.
    pub const HELPER_V2_POP_SCOPE: &str = "helper_v2_pop_scope";
    /// `helper_v2_pop_scope` function pointer type.
    pub type PopScopeFn = unsafe extern "C" fn(*mut EvalCtx);

    /// Name of the `helper_v2_drop_value` runtime function.
    pub const HELPER_V2_DROP_VALUE: &str = "helper_v2_drop_value";
    /// `helper_v2_drop_value` function pointer type.
    pub type DropValueFn = unsafe extern "C" fn(*mut Value);

    /// Name of the `helper_v2_apply_closure` runtime function.
    pub const HELPER_V2_APPLY_CLOSURE: &str = "helper_v2_apply_closure";
    /// `helper_v2_apply_closure` function pointer type.
    pub type ApplyClosureFn =
        unsafe extern "C" fn(*mut EvalCtx, *mut Value, *const *mut Value, usize) -> *mut Value;

    /// Name of the `helper_v2_call_foreign` runtime function.
    pub const HELPER_V2_CALL_FOREIGN: &str = "helper_v2_call_foreign";
    /// `helper_v2_call_foreign` function pointer type.
    pub type CallForeignFn =
        unsafe extern "C" fn(*mut EvalCtx, usize, *const *mut Value, usize) -> *mut Value;

    /// Name of the `helper_v2_pair` runtime function.
    pub const HELPER_V2_PAIR: &str = "helper_v2_pair";
    /// `helper_v2_pair` function pointer type.
    pub type PairFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_unpair_fst` runtime function.
    pub const HELPER_V2_UNPAIR_FST: &str = "helper_v2_unpair_fst";
    /// `helper_v2_unpair_fst` function pointer type.
    pub type UnpairFstFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_unpair_snd` runtime function.
    pub const HELPER_V2_UNPAIR_SND: &str = "helper_v2_unpair_snd";
    /// `helper_v2_unpair_snd` function pointer type.
    pub type UnpairSndFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_either_left` runtime function.
    pub const HELPER_V2_EITHER_LEFT: &str = "helper_v2_either_left";
    /// `helper_v2_either_left` function pointer type.
    pub type EitherLeftFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_either_right` runtime function.
    pub const HELPER_V2_EITHER_RIGHT: &str = "helper_v2_either_right";
    /// `helper_v2_either_right` function pointer type.
    pub type EitherRightFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_either_is_left` runtime function.
    pub const HELPER_V2_EITHER_IS_LEFT: &str = "helper_v2_either_is_left";
    /// `helper_v2_either_is_left` function pointer type.
    pub type EitherIsLeftFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> u8;

    /// Name of the `helper_v2_either_value` runtime function.
    pub const HELPER_V2_EITHER_VALUE: &str = "helper_v2_either_value";
    /// `helper_v2_either_value` function pointer type.
    pub type EitherValueFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> *mut Value;

    /// Name of the `helper_v2_derived_step_arg_incoming` runtime function.
    pub const HELPER_V2_DERIVED_STEP_ARG_INCOMING: &str = "helper_v2_derived_step_arg_incoming";
    /// `helper_v2_derived_step_arg_incoming` function pointer type.
    pub type DerivedStepArgIncomingFn =
        unsafe extern "C" fn(*mut EvalCtx, *mut Value, usize) -> *mut Value;

    /// Name of the `helper_v2_apply_native` runtime function.
    pub const HELPER_V2_APPLY_NATIVE: &str = "helper_v2_apply_native";
    /// `helper_v2_apply_native` function pointer type.
    pub type ApplyNativeFn = unsafe extern "C" fn(
        *mut EvalCtx,
        usize,
        *const *mut Value,
        usize,
        *mut Value,
    ) -> *mut Value;

    /// Name of the `helper_v2_int_arith` runtime function.
    pub const HELPER_V2_INT_ARITH: &str = "helper_v2_int_arith";
    /// `helper_v2_int_arith` function pointer type.
    pub type IntArithFn = unsafe extern "C" fn(
        *mut EvalCtx,
        usize,
        usize,
        *mut Value,
        *mut Value,
        *mut Value,
    ) -> *mut Value;

    /// Name of the `helper_v2_loop_check` runtime function.
    pub const HELPER_V2_LOOP_CHECK: &str = "helper_v2_loop_check";
    /// `helper_v2_loop_check` function pointer type.
    pub type LoopCheckFn = unsafe extern "C" fn(*mut EvalCtx, *mut Value) -> u8;

    /// All registered `helper_v2_*` names, in canonical order.
    pub const ALL: &[&str] = &[
        HELPER_V2_CLONE,
        HELPER_V2_CALL_FFI_RAW,
        HELPER_V2_PUSH_LITERAL,
        HELPER_V2_LOAD_VAR,
        HELPER_V2_LOAD_PAYLOAD_VAR,
        HELPER_V2_LOAD_PAYLOAD,
        HELPER_V2_EVAL_CHILD,
        HELPER_V2_EXTEND_ENV,
        HELPER_V2_EXTEND_ENV_PAYLOAD,
        HELPER_V2_POP_SCOPE,
        HELPER_V2_DROP_VALUE,
        HELPER_V2_APPLY_CLOSURE,
        HELPER_V2_CALL_FOREIGN,
        HELPER_V2_PAIR,
        HELPER_V2_UNPAIR_FST,
        HELPER_V2_UNPAIR_SND,
        HELPER_V2_EITHER_LEFT,
        HELPER_V2_EITHER_RIGHT,
        HELPER_V2_EITHER_IS_LEFT,
        HELPER_V2_EITHER_VALUE,
        HELPER_V2_DERIVED_STEP_ARG_INCOMING,
        HELPER_V2_APPLY_NATIVE,
        HELPER_V2_INT_ARITH,
        HELPER_V2_LOOP_CHECK,
    ];
}

#[cfg(test)]
mod tests;
