//! Rust `Function` impls for the `str` module — the handls (handlers)
//! for the `op`/`handl` declarations in `str.hnl`.
//!
//! All 45 functions are FFI atoms: they call Rust `String`/`str` methods
//! that cannot be expressed as pure handl arrows without losing the exact
//! `TypeMismatch` error messages the tests rely on. The `op`/`handl`
//! declaration form (ADR-0077 Phase 2) declares the interface in
//! `str.hnl`; these impls provide the logic and are registered in an
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use handl_platform::{EvalContext, EvalError, Function};

use crate::shared::{b, num};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_string(arg: &serde_json::Value) -> Result<&str, EvalError> {
    arg.as_str()
        .ok_or(handl_platform::EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args
        .first()
        .ok_or(handl_platform::EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
    require_string(arg)
}

fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(handl_platform::EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let a = require_string(&args[0])?;
    let b = require_string(&args[1])?;
    Ok((a, b))
}

fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}

macro_rules! string_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let input = require_one_string(args)?;
                Ok(s($body(input)))
            }
        }
    };
}

#[allow(unused_imports)]
use string_unary_fn;

macro_rules! string_predicate_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let (input, pattern) = require_two_strings(args)?;
                Ok(b($body(input, pattern)))
            }
        }
    };
}

#[allow(unused_imports)]
use string_predicate_fn;

// ---------------------------------------------------------------------------
// Basic transforms
// ---------------------------------------------------------------------------

string_unary_fn!(Upper, |input: &str| input.to_uppercase());
string_unary_fn!(Lower, |input: &str| input.to_lowercase());
string_unary_fn!(Trim, |input: &str| input.trim().to_string());
string_unary_fn!(TrimStart, |input: &str| input.trim_start().to_string());
string_unary_fn!(TrimEnd, |input: &str| input.trim_end().to_string());
string_unary_fn!(Reverse, |input: &str| input
    .chars()
    .rev()
    .collect::<String>());

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
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let mut result = String::new();
        for arg in args {
            let s = require_string(arg)?;
            result.push_str(s);
        }
        Ok(s(result))
    }
}

string_predicate_fn!(Equals, |input: &str, other: &str| input == other);

#[derive(Clone, Copy)]
pub struct IsEmpty;
impl Function for IsEmpty {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        Ok(b(input.is_empty()))
    }
}

#[derive(Clone, Copy)]
pub struct TrimMatches;
impl Function for TrimMatches {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, chars) = require_two_strings(args)?;
        if chars.is_empty() {
            return Ok(s(input.to_string()));
        }
        let char_set: Vec<char> = chars.chars().collect();
        Ok(s(input.trim_matches(|c| char_set.contains(&c)).to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct IsAlpha;
impl Function for IsAlpha {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        Ok(b(
            !input.is_empty() && input.chars().all(|c| c.is_alphabetic())
        ))
    }
}

#[derive(Clone, Copy)]
pub struct IsNumeric;
impl Function for IsNumeric {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        Ok(b(!input.is_empty() && input.chars().all(|c| c.is_numeric())))
    }
}

#[derive(Clone, Copy)]
pub struct IsAlphanumeric;
impl Function for IsAlphanumeric {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        Ok(b(
            !input.is_empty() && input.chars().all(|c| c.is_alphanumeric())
        ))
    }
}

// ---------------------------------------------------------------------------
// Case conversion
// ---------------------------------------------------------------------------

string_unary_fn!(Capitalize, |input: &str| {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::new();
            result.extend(first.to_uppercase());
            result.extend(chars.flat_map(|c| c.to_lowercase()));
            result
        }
    }
});

string_unary_fn!(TitleCase, |input: &str| {
    let mut result = String::new();
    let mut new_word = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            result.push(ch);
            new_word = true;
        } else if new_word {
            result.extend(ch.to_uppercase());
            new_word = false;
        } else {
            result.extend(ch.to_lowercase());
        }
    }
    result
});

fn split_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else if ch.is_uppercase() {
            if !current.is_empty() && current.chars().last().map_or(false, |c| c.is_lowercase()) {
                words.push(std::mem::take(&mut current));
            }
            current.push(ch);
        } else {
            if !current.is_empty()
                && current.chars().last().map_or(false, |c| c.is_uppercase())
                && current.chars().count() > 1
            {
                let last = current.pop().unwrap();
                words.push(std::mem::take(&mut current));
                current.push(last);
            }
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.into_iter().map(|w| w.to_lowercase()).collect()
}

string_unary_fn!(SnakeCase, |input: &str| split_words(input).join("_"));
string_unary_fn!(KebabCase, |input: &str| split_words(input).join("-"));

string_unary_fn!(CamelCase, |input: &str| {
    let words = split_words(input);
    let mut result = String::new();
    for (i, word) in words.iter().enumerate() {
        if i == 0 {
            result.push_str(word);
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                result.extend(first.to_uppercase());
                result.extend(chars);
            }
        }
    }
    result
});

string_unary_fn!(PascalCase, |input: &str| {
    let words = split_words(input);
    let mut result = String::new();
    for word in &words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            result.extend(first.to_uppercase());
            result.extend(chars);
        }
    }
    result
});

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

string_predicate_fn!(Contains, |input: &str, pat: &str| input.contains(pat));
string_predicate_fn!(StartsWith, |input: &str, pat: &str| input.starts_with(pat));
string_predicate_fn!(EndsWith, |input: &str, pat: &str| input.ends_with(pat));

#[derive(Clone, Copy)]
pub struct Count;
impl Function for Count {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, pattern) = require_two_strings(args)?;
        Ok(num(input.matches(pattern).count() as f64))
    }
}

#[derive(Clone, Copy)]
pub struct Find;
impl Function for Find {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, pattern) = require_two_strings(args)?;
        let idx = input
            .find(pattern)
            .map(|byte_pos| input[..byte_pos].chars().count())
            .map(|i| i as f64)
            .unwrap_or(-1.0);
        Ok(num(idx))
    }
}

#[derive(Clone, Copy)]
pub struct RFind;
impl Function for RFind {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, pattern) = require_two_strings(args)?;
        let idx = input
            .rfind(pattern)
            .map(|byte_pos| input[..byte_pos].chars().count())
            .map(|i| i as f64)
            .unwrap_or(-1.0);
        Ok(num(idx))
    }
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr: Vec<usize> = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[derive(Clone, Copy)]
pub struct Levenshtein;
impl Function for Levenshtein {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_strings(args)?;
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let dist = levenshtein(&a_chars, &b_chars);
        Ok(num(dist as f64))
    }
}

#[derive(Clone, Copy)]
pub struct Similarity;
impl Function for Similarity {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_strings(args)?;
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let dist = levenshtein(&a_chars, &b_chars);
        let max_len = a_chars.len().max(b_chars.len());
        if max_len == 0 {
            return Ok(num(1.0));
        }
        Ok(num(1.0 - (dist as f64 / max_len as f64)))
    }
}

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Split;
impl Function for Split {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, sep) = require_two_strings(args)?;
        let parts: Vec<serde_json::Value> = input
            .split(sep)
            .map(|part| serde_json::Value::String(part.to_string()))
            .collect();
        Ok(serde_json::Value::from(serde_json::Value::Array(parts)))
    }
}

#[derive(Clone, Copy)]
pub struct Join;
impl Function for Join {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let sep = require_string(&args[1])?;
        let arr = args[0].as_array().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array",
        )))?;
        let parts: Vec<&str> = arr
            .iter()
            .map(|v| {
                v.as_str().ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(s(parts.join(sep)))
    }
}

#[derive(Clone, Copy)]
pub struct Replace;
impl Function for Replace {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let from = require_string(&args[1])?;
        let to = require_string(&args[2])?;
        Ok(s(input.replace(from, to)))
    }
}

#[derive(Clone, Copy)]
pub struct ReplaceFirst;
impl Function for ReplaceFirst {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let from = require_string(&args[1])?;
        let to = require_string(&args[2])?;
        Ok(s(input.replacen(from, to, 1)))
    }
}

#[derive(Clone, Copy)]
pub struct Repeat;
impl Function for Repeat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let count = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if count < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        if !count.is_finite() || count > 1_000_000.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "finite number <= 1_000_000", "non-finite or excessively large number",
            )));
        }
        Ok(s(input.repeat(count as usize)))
    }
}

#[derive(Clone, Copy)]
pub struct CharAt;
impl Function for CharAt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let index = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if index < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let char_count = input.chars().count();
        if (index as usize) >= char_count {
            return Err(EvalError::Other(format!(
                "index {index} out of bounds (char count: {char_count})"
            )));
        }
        Ok(s(input.chars().nth(index as usize).unwrap().to_string()))
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
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let start = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        let end = args[2].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if start < 0.0 || end < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let chars: Vec<char> = input.chars().collect();
        let end = (end as usize).min(chars.len());
        let start = (start as usize).min(end);
        Ok(s(chars[start..end].iter().collect::<String>()))
    }
}

fn pad_to_len(input: &str, target_len: usize, pad_char: &str, pad_left: bool) -> String {
    let char_count = input.chars().count();
    if char_count >= target_len || pad_char.is_empty() {
        return input.to_string();
    }
    let pad_total = target_len - char_count;
    let pad_chars: Vec<char> = pad_char.chars().collect();
    let pad_repeated: String = pad_chars.iter().cycle().take(pad_total).collect();
    if pad_left {
        format!("{pad_repeated}{input}")
    } else {
        format!("{input}{pad_repeated}")
    }
}

#[derive(Clone, Copy)]
pub struct PadLeft;
impl Function for PadLeft {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let target_len = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if target_len < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        if !target_len.is_finite() || target_len > 1_000_000.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "finite number <= 1_000_000", "non-finite or excessively large number",
            )));
        }
        let pad_char = require_string(&args[2])?;
        Ok(s(pad_to_len(input, target_len as usize, pad_char, true)))
    }
}

#[derive(Clone, Copy)]
pub struct PadRight;
impl Function for PadRight {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let target_len = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if target_len < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        if !target_len.is_finite() || target_len > 1_000_000.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "finite number <= 1_000_000", "non-finite or excessively large number",
            )));
        }
        let pad_char = require_string(&args[2])?;
        Ok(s(pad_to_len(input, target_len as usize, pad_char, false)))
    }
}

#[derive(Clone, Copy)]
pub struct StripPrefix;
impl Function for StripPrefix {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, prefix) = require_two_strings(args)?;
        Ok(s(input.strip_prefix(prefix).unwrap_or(input).to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct StripSuffix;
impl Function for StripSuffix {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (input, suffix) = require_two_strings(args)?;
        Ok(s(input.strip_suffix(suffix).unwrap_or(input).to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct Lines;
impl Function for Lines {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let lines: Vec<serde_json::Value> = if input.is_empty() {
            vec![serde_json::Value::String(String::new())]
        } else {
            input
                .lines()
                .map(|line| serde_json::Value::String(line.to_string()))
                .collect()
        };
        Ok(serde_json::Value::from(serde_json::Value::Array(lines)))
    }
}

#[derive(Clone, Copy)]
pub struct SplitAt;
impl Function for SplitAt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let index = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if index < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let chars: Vec<char> = input.chars().collect();
        let mid = (index as usize).min(chars.len());
        let left: String = chars[..mid].iter().collect();
        let right: String = chars[mid..].iter().collect();
        let parts = vec![
            serde_json::Value::String(left),
            serde_json::Value::String(right),
        ];
        Ok(serde_json::Value::from(serde_json::Value::Array(parts)))
    }
}

#[derive(Clone, Copy)]
pub struct SplitWhitespace;
impl Function for SplitWhitespace {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let parts: Vec<serde_json::Value> = input
            .split_whitespace()
            .map(|part| serde_json::Value::String(part.to_string()))
            .collect();
        Ok(serde_json::Value::from(serde_json::Value::Array(parts)))
    }
}

#[derive(Clone, Copy)]
pub struct Truncate;
impl Function for Truncate {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let max_len = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if max_len < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let max_len = max_len as usize;
        let suffix = if args.len() >= 3 {
            require_string(&args[2])?
        } else {
            "..."
        };
        let char_count = input.chars().count();
        if char_count <= max_len {
            return Ok(s(input.to_string()));
        }
        let suffix_len = suffix.chars().count();
        if max_len < suffix_len {
            let truncated: String = input.chars().take(max_len).collect();
            return Ok(s(truncated));
        }
        let take = max_len - suffix_len;
        let truncated: String = input.chars().take(take).collect();
        Ok(s(format!("{truncated}{suffix}")))
    }
}

#[derive(Clone, Copy)]
pub struct Slugify;
impl Function for Slugify {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_string(args)?;
        let slug: String = input
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else if c.is_whitespace() || c == '-' || c == '_' {
                    '-'
                } else {
                    '\0'
                }
            })
            .filter(|c| *c != '\0')
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let mut result = String::new();
        let mut prev_dash = false;
        for ch in slug.chars() {
            if ch == '-' {
                if !prev_dash {
                    result.push(ch);
                    prev_dash = true;
                }
            } else {
                result.push(ch);
                prev_dash = false;
            }
        }
        Ok(s(result))
    }
}

#[derive(Clone, Copy)]
pub struct WordWrap;
impl Function for WordWrap {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let input = require_string(&args[0])?;
        let width = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if width < 1.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "positive number", "non-positive number",
            )));
        }
        let width = width as usize;
        let mut result = String::new();
        for line in input.lines() {
            if line.is_empty() {
                result.push('\n');
                continue;
            }
            let mut current_len = 0usize;
            for word in line.split_whitespace() {
                let word_len = word.chars().count();
                if current_len == 0 {
                    result.push_str(word);
                    current_len = word_len;
                } else if current_len + 1 + word_len <= width {
                    result.push(' ');
                    result.push_str(word);
                    current_len += 1 + word_len;
                } else {
                    result.push('\n');
                    result.push_str(word);
                    current_len = word_len;
                }
            }
            result.push('\n');
        }
        if result.ends_with('\n') {
            result.pop();
        }
        Ok(s(result))
    }
}

use std::sync::Arc;
use handl_plugin_lib_handl::OpRegistry;

pub const STR_OPS: &[&str] = &[
    "str.upper",
    "str.lower",
    "str.trim",
    "str.trim_start",
    "str.trim_end",
    "str.trim_matches",
    "str.reverse",
    "str.length",
    "str.concat",
    "str.is_empty",
    "str.equals",
    "str.is_alpha",
    "str.is_numeric",
    "str.is_alphanumeric",
    "str.capitalize",
    "str.title_case",
    "str.snake_case",
    "str.camel_case",
    "str.pascal_case",
    "str.kebab_case",
    "str.contains",
    "str.starts_with",
    "str.ends_with",
    "str.count",
    "str.find",
    "str.rfind",
    "str.levenshtein",
    "str.similarity",
    "str.split",
    "str.join",
    "str.replace",
    "str.replace_first",
    "str.repeat",
    "str.char_at",
    "str.slice",
    "str.pad_left",
    "str.pad_right",
    "str.strip_prefix",
    "str.strip_suffix",
    "str.lines",
    "str.split_at",
    "str.split_whitespace",
    "str.truncate",
    "str.slugify",
    "str.word_wrap",
];

pub fn str_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("str.upper", Arc::new(Upper));
    registry.register("str.lower", Arc::new(Lower));
    registry.register("str.trim", Arc::new(Trim));
    registry.register("str.trim_start", Arc::new(TrimStart));
    registry.register("str.trim_end", Arc::new(TrimEnd));
    registry.register("str.trim_matches", Arc::new(TrimMatches));
    registry.register("str.reverse", Arc::new(Reverse));
    registry.register("str.length", Arc::new(Length));
    registry.register("str.concat", Arc::new(Concat));
    registry.register("str.is_empty", Arc::new(IsEmpty));
    registry.register("str.equals", Arc::new(Equals));
    registry.register("str.is_alpha", Arc::new(IsAlpha));
    registry.register("str.is_numeric", Arc::new(IsNumeric));
    registry.register("str.is_alphanumeric", Arc::new(IsAlphanumeric));
    registry.register("str.capitalize", Arc::new(Capitalize));
    registry.register("str.title_case", Arc::new(TitleCase));
    registry.register("str.snake_case", Arc::new(SnakeCase));
    registry.register("str.camel_case", Arc::new(CamelCase));
    registry.register("str.pascal_case", Arc::new(PascalCase));
    registry.register("str.kebab_case", Arc::new(KebabCase));
    registry.register("str.contains", Arc::new(Contains));
    registry.register("str.starts_with", Arc::new(StartsWith));
    registry.register("str.ends_with", Arc::new(EndsWith));
    registry.register("str.count", Arc::new(Count));
    registry.register("str.find", Arc::new(Find));
    registry.register("str.rfind", Arc::new(RFind));
    registry.register("str.levenshtein", Arc::new(Levenshtein));
    registry.register("str.similarity", Arc::new(Similarity));
    registry.register("str.split", Arc::new(Split));
    registry.register("str.join", Arc::new(Join));
    registry.register("str.replace", Arc::new(Replace));
    registry.register("str.replace_first", Arc::new(ReplaceFirst));
    registry.register("str.repeat", Arc::new(Repeat));
    registry.register("str.char_at", Arc::new(CharAt));
    registry.register("str.slice", Arc::new(Slice));
    registry.register("str.pad_left", Arc::new(PadLeft));
    registry.register("str.pad_right", Arc::new(PadRight));
    registry.register("str.strip_prefix", Arc::new(StripPrefix));
    registry.register("str.strip_suffix", Arc::new(StripSuffix));
    registry.register("str.lines", Arc::new(Lines));
    registry.register("str.split_at", Arc::new(SplitAt));
    registry.register("str.split_whitespace", Arc::new(SplitWhitespace));
    registry.register("str.truncate", Arc::new(Truncate));
    registry.register("str.slugify", Arc::new(Slugify));
    registry.register("str.word_wrap", Arc::new(WordWrap));
    registry
}
