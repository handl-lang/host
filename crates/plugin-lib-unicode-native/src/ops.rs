//! Rust `Function` impls for the native `unicode` module.
//!
//! Table-dependent operations are backed by ICU4X and `unicode-width`:
//! - grapheme clusters / UAX #29 via `icu_segmenter`
//! - canonical normalization (NFC/NFD) / UAX #15 via `icu_normalizer`
//! - full case folding via `icu_casemap`
//! - display width / UAX #11 via `unicode-width`
//!
//! Code-point interop, identifier validation, and UTF-8 decode are shared
//! with `handl-plugin-lib-unicode`.

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_unicode::decode::Utf8Iterator;
use icu_casemap::CaseMapperBorrowed;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use icu_segmenter::GraphemeClusterSegmenter;
use serde_json::Value as JsonValue;
use unicode_width::UnicodeWidthStr;

fn require_string(arg: &JsonValue) -> Result<&str, EvalError> {
    arg.as_str()
        .ok_or_else(|| EvalError::TypeMismatch("expected string, got non-string".into()))
}

fn require_one_string(args: &[JsonValue]) -> Result<&str, EvalError> {
    let arg = args
        .first()
        .ok_or_else(|| EvalError::TypeMismatch("expected 1 argument, got 0".into()))?;
    require_string(arg)
}

fn require_int(arg: &JsonValue, what: &str) -> Result<usize, EvalError> {
    arg.as_f64()
        .filter(|n| *n >= 0.0 && n.fract() == 0.0 && *n <= usize::MAX as f64)
        .map(|n| n as usize)
        .ok_or_else(|| EvalError::TypeMismatch(format!("expected {what}, got non-integer")))
}

fn require_string_and_index(args: &[JsonValue]) -> Result<(&str, usize), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected 2 arguments, got {}",
            args.len()
        )));
    }
    let s = require_string(&args[0])?;
    let i = require_int(&args[1], "non-negative integer index")?;
    Ok((s, i))
}

fn str_val(value: impl Into<String>) -> JsonValue {
    JsonValue::String(value.into())
}

fn b(value: bool) -> JsonValue {
    JsonValue::Bool(value)
}

fn n(value: u64) -> JsonValue {
    JsonValue::Number(serde_json::Number::from(value))
}

// ---------------------------------------------------------------------------
// Code-point interop
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct CodePoints;
impl Function for CodePoints {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let points: Vec<JsonValue> = s
            .chars()
            .map(|c| JsonValue::Number(serde_json::Number::from(c as u32)))
            .collect();
        Ok(JsonValue::Array(points))
    }
}

#[derive(Clone, Copy)]
pub struct FromCodePoints;
impl Function for FromCodePoints {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let arr = args
            .first()
            .and_then(JsonValue::as_array)
            .ok_or_else(|| EvalError::TypeMismatch("expected array of code points".into()))?;
        let mut out = String::with_capacity(arr.len());
        for v in arr {
            let n = v
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("expected numeric code point".into()))?;
            if n < 0.0 || n.fract() != 0.0 || n > u32::MAX as f64 {
                return Err(EvalError::TypeMismatch(format!(
                    "code point must be a non-negative integer, got {n}"
                )));
            }
            let cp = n as u32;
            match char::from_u32(cp) {
                Some(c) => out.push(c),
                None => {
                    return Err(EvalError::Other(format!(
                        "invalid Unicode scalar value: {cp}"
                    )));
                }
            }
        }
        Ok(str_val(out))
    }
}

#[derive(Clone, Copy)]
pub struct IsAscii;
impl Function for IsAscii {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        Ok(b(s.is_ascii()))
    }
}

#[derive(Clone, Copy)]
pub struct CodePointAt;
impl Function for CodePointAt {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let (s, i) = require_string_and_index(args)?;
        match s.chars().nth(i) {
            Some(c) => Ok(JsonValue::Number(serde_json::Number::from(c as u32))),
            None => Ok(JsonValue::Null),
        }
    }
}

// ---------------------------------------------------------------------------
// UTF-8 decode
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Decode;
impl Function for Decode {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let mut pairs = Vec::new();
        for item in Utf8Iterator::new(s.as_bytes()) {
            let (offset, ch) =
                item.map_err(|_| EvalError::Other("invalid UTF-8 in string".to_string()))?;
            pairs.push(JsonValue::Array(vec![
                JsonValue::Number(serde_json::Number::from(offset)),
                JsonValue::Number(serde_json::Number::from(ch as u32)),
            ]));
        }
        Ok(JsonValue::Array(pairs))
    }
}

// ---------------------------------------------------------------------------
// Identifier validation (UAX #31)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct IsValidIdentifier;
impl Function for IsValidIdentifier {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        Ok(b(handl_plugin_lib_unicode::is_identifier(s)))
    }
}

#[derive(Clone, Copy)]
pub struct IsXidStart;
impl Function for IsXidStart {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let first = s.chars().next();
        Ok(b(first.map_or(
            false,
            handl_plugin_lib_unicode::is_identifier_start,
        )))
    }
}

#[derive(Clone, Copy)]
pub struct IsXidContinue;
impl Function for IsXidContinue {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let first = s.chars().next();
        Ok(b(first.map_or(
            false,
            handl_plugin_lib_unicode::is_identifier_continue,
        )))
    }
}

// ---------------------------------------------------------------------------
// Grapheme clusters (UAX #29)
// ---------------------------------------------------------------------------

fn grapheme_boundaries(s: &str) -> Vec<usize> {
    let segmenter = GraphemeClusterSegmenter::new();
    segmenter.segment_str(s).collect()
}

#[derive(Clone, Copy)]
pub struct Graphemes;
impl Function for Graphemes {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let boundaries = grapheme_boundaries(s);
        let clusters: Vec<JsonValue> = boundaries
            .windows(2)
            .map(|w| str_val(&s[w[0]..w[1]]))
            .collect();
        Ok(JsonValue::Array(clusters))
    }
}

#[derive(Clone, Copy)]
pub struct GraphemeCount;
impl Function for GraphemeCount {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let boundaries = grapheme_boundaries(s);
        let count = boundaries.len().saturating_sub(1);
        Ok(n(count as u64))
    }
}

// ---------------------------------------------------------------------------
// Display width (UAX #11)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Width;
impl Function for Width {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        Ok(n(s.width() as u64))
    }
}

// ---------------------------------------------------------------------------
// Case folding
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Casefold;
impl Function for Casefold {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let cm = CaseMapperBorrowed::new();
        Ok(str_val(cm.fold_string(s).into_owned()))
    }
}

// ---------------------------------------------------------------------------
// Normalization (UAX #15)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Nfc;
impl Function for Nfc {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let nfc = ComposingNormalizerBorrowed::new_nfc();
        Ok(str_val(nfc.normalize(s).into_owned()))
    }
}

#[derive(Clone, Copy)]
pub struct Nfd;
impl Function for Nfd {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let s = require_one_string(args)?;
        let nfd = DecomposingNormalizerBorrowed::new_nfd();
        Ok(str_val(nfd.normalize(s).into_owned()))
    }
}
