//! Rust `Function` impls for the `char` module — the handls (handlers)
//! for the `op`/`handl` declarations in `char-native.hnl`.
//!
//! All 18 functions are FFI atoms over `std::char`: category predicates
//! (`is_whitespace`, `is_alphabetic`, `is_numeric`, `is_alphanumeric`,
//! `is_uppercase`, `is_lowercase`), case maps (`to_uppercase`,
//! `to_lowercase`, `to_titlecase` via `to_uppercase` — Rust has no
//! titlecase), scalar conversion (`from_u32`, `as u32`), and
//! char-sequence access (`chars().nth`, `chars().skip/take`, `count`,
//! `concat`) with *character* indices, never byte offsets.
//!
//! `chars`/`from_chars` are O(n) bulk split/join so `str-handl`
//! algorithms iterate char lists without quadratic `at` scans.

use handl_platform::{EvalContext, EvalError, Function};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_string(arg: &serde_json::Value) -> Result<&str, EvalError> {
    arg.as_str()
        .ok_or(EvalError::TypeMismatch("expected string, got non-string".into()))
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args
        .first()
        .ok_or(EvalError::TypeMismatch("expected 1 args, got 0".into()))?;
    require_string(arg)
}

fn require_int(arg: &serde_json::Value, what: &str) -> Result<usize, EvalError> {
    arg.as_f64()
        .filter(|n| *n >= 0.0 && n.fract() == 0.0)
        .map(|n| n as usize)
        .ok_or(EvalError::TypeMismatch(format!("expected {what}, got non-integer")))
}

fn require_char_index(args: &[serde_json::Value], pos: usize) -> Result<usize, EvalError> {
    let arg = args
        .get(pos)
        .ok_or(EvalError::TypeMismatch(format!("expected {pos}+ args")))?;
    require_int(arg, "non-negative integer index")
}

fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}

fn b(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

fn num(value: f64) -> serde_json::Value {
    serde_json::json!(value)
}

fn type_mismatch(expected: &str, got: &str) -> EvalError {
    EvalError::TypeMismatch(format!("expected {expected}, got {got}"))
}

/// The single char of a 1-char string, or an error for empty/multi-char.
fn require_one_char(args: &[serde_json::Value]) -> Result<char, EvalError> {
    let input = require_one_string(args)?;
    let mut it = input.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(EvalError::Other(format!(
            "expected a single char, got {input:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Character classification atoms
// ---------------------------------------------------------------------------

macro_rules! char_pred_fn {
    ($name:ident, $pred:expr) => {
        #[derive(Clone, Copy)]
        pub struct $name;
        impl Function for $name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let c = require_one_char(args)?;
                let pred: fn(char) -> bool = $pred;
                Ok(b(pred(c)))
            }
        }
    };
}

char_pred_fn!(IsWhitespace, |c: char| c.is_whitespace());
char_pred_fn!(IsAlpha, |c: char| c.is_alphabetic());
char_pred_fn!(IsNumeric, |c: char| c.is_numeric());
char_pred_fn!(IsAlnum, |c: char| c.is_alphanumeric());
char_pred_fn!(IsUppercase, |c: char| c.is_uppercase());
char_pred_fn!(IsLowercase, |c: char| c.is_lowercase());

// ---------------------------------------------------------------------------
// Case-mapping atoms (may expand: ß → SS)
// ---------------------------------------------------------------------------

macro_rules! char_map_fn {
    ($name:ident, $map:expr) => {
        #[derive(Clone, Copy)]
        pub struct $name;
        impl Function for $name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let c = require_one_char(args)?;
                let map: fn(char) -> String = $map;
                Ok(s(map(c)))
            }
        }
    };
}

char_map_fn!(ToUpper, |c: char| c.to_uppercase().collect());
char_map_fn!(ToLower, |c: char| c.to_lowercase().collect());
// Rust std::char has no to_titlecase; fall back to uppercase (titlecase
// digraphs like ǲ require an ICU backend).
char_map_fn!(ToTitle, |c: char| c.to_uppercase().collect());

// ---------------------------------------------------------------------------
// Scalar-value conversion atoms
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct CodePoint;
impl Function for CodePoint {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let c = require_one_char(args)?;
        Ok(num(c as u32 as f64))
    }
}

#[derive(Clone, Copy)]
pub struct FromCodePoint;
impl Function for FromCodePoint {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args
            .first()
            .ok_or_else(|| type_mismatch("1 args", "0"))?;
        let n = arg
            .as_f64()
            .filter(|n| *n >= 0.0 && n.fract() == 0.0)
            .ok_or_else(|| type_mismatch("non-negative integer", "non-integer"))?;
        let cp = n as u32;
        char::from_u32(cp)
            .map(|c| s(c.to_string()))
            .ok_or_else(|| EvalError::Other(format!("invalid code point {cp}")))
    }
}

// ---------------------------------------------------------------------------
// Char-sequence access atoms (char indices, not byte offsets)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct At;
impl Function for At {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let index = require_char_index(args, 1)?;
        let char_count = input.chars().count();
        input
            .chars()
            .nth(index)
            .map(|c| s(c.to_string()))
            .ok_or_else(|| {
                EvalError::Other(format!(
                    "index {index} out of bounds (char count: {char_count})"
                ))
            })
    }
}

#[derive(Clone, Copy)]
pub struct Slice;
impl Function for Slice {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let start = require_char_index(args, 1)?;
        let end = require_char_index(args, 2)?;
        Ok(s(input.chars().skip(start).take(end.saturating_sub(start)).collect::<String>()))
    }
}

#[derive(Clone, Copy)]
pub struct Length;
impl Function for Length {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        Ok(num(input.chars().count() as f64))
    }
}

#[derive(Clone, Copy)]
pub struct Concat;
impl Function for Concat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let a = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| type_mismatch("string", "non-string"))?;
        let b = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| type_mismatch("string", "non-string"))?;
        Ok(s(format!("{a}{b}")))
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
        let input = require_one_string(args)?;
        let index = require_char_index(args, 1)?;
        let char_count = input.chars().count();
        input
            .chars()
            .nth(index)
            .map(|c| num(c as u32 as f64))
            .ok_or_else(|| {
                EvalError::Other(format!(
                    "index {index} out of bounds (char count: {char_count})"
                ))
            })
    }
}

// ---------------------------------------------------------------------------
// Bulk char-sequence conversion atoms (O(n) split/join)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Chars;
impl Function for Chars {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let chars: Vec<serde_json::Value> = input
            .chars()
            .map(|c| serde_json::Value::String(c.to_string()))
            .collect();
        Ok(serde_json::Value::Array(chars))
    }
}

#[derive(Clone, Copy)]
pub struct FromChars;
impl Function for FromChars {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args
            .first()
            .ok_or_else(|| type_mismatch("1 args", "0"))?;
        let arr = arg
            .as_array()
            .ok_or_else(|| type_mismatch("array", "non-array"))?;
        let mut result = String::new();
        for v in arr {
            result.push_str(require_string(v)?);
        }
        Ok(s(result))
    }
}
