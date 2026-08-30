use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use std::str::FromStr;
use handl_platform::{EvalError};

use crate::DatetimeError;

pub(super) fn require_string(arg: &serde_json::Value) -> Result<&str, EvalError> {
    arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}

pub(super) fn require_number(arg: &serde_json::Value) -> Result<f64, EvalError> {
    arg.as_f64().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "number", "non-number",)))
}

pub(super) fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
    require_string(arg)
}

pub(super) fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let a = require_string(&args[0])?;
    let b = require_string(&args[1])?;
    Ok((a, b))
}

pub(super) fn require_string_and_number(args: &[serde_json::Value]) -> Result<(&str, f64), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let s = require_string(&args[0])?;
    let n = require_number(&args[1])?;
    Ok((s, n))
}

pub(super) fn require_three_strings(
    args: &[serde_json::Value],
) -> Result<(&str, &str, &str), EvalError> {
    if args.len() < 3 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 3, args.len(),)));
    }
    let a = require_string(&args[0])?;
    let b = require_string(&args[1])?;
    let c = require_string(&args[2])?;
    Ok((a, b, c))
}

pub(super) fn parse_iso(s: &str) -> Result<DateTime<Utc>, EvalError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
        .map_err(|_| {
            DatetimeError::InvalidDatetime {
                input: s.to_string(),
            }
            .into_vm_error()
        })
}

pub(super) fn parse_with_format_and_offset(
    s: &str,
    fmt: &str,
    offset: Option<&str>,
) -> Result<DateTime<Utc>, EvalError> {
    let parsed = chrono::NaiveDateTime::parse_from_str(s, fmt)
        .map(Some)
        .or_else(|_| chrono::NaiveDate::parse_from_str(s, fmt).map(|d| d.and_hms_opt(0, 0, 0)))
        .map_err(|_| {
            DatetimeError::ParseFailed {
                input: s.to_string(),
                format: fmt.to_string(),
            }
            .into_vm_error()
        })?;

    let ndt = parsed.ok_or(
        DatetimeError::ParseFailed {
            input: s.to_string(),
            format: fmt.to_string(),
        }
        .into_vm_error(),
    )?;

    match offset {
        Some(off_str) => {
            let off = parse_offset(off_str)?;
            Ok(off
                .from_local_datetime(&ndt)
                .single()
                .ok_or(
                    DatetimeError::ParseFailed {
                        input: s.to_string(),
                        format: fmt.to_string(),
                    }
                    .into_vm_error(),
                )?
                .with_timezone(&Utc))
        }
        None => Ok(ndt.and_utc()),
    }
}

pub(super) fn parse_offset(offset_str: &str) -> Result<FixedOffset, EvalError> {
    if offset_str.eq_ignore_ascii_case("Z") || offset_str == "+00:00" || offset_str == "-00:00" {
        return Ok(FixedOffset::east_opt(0).unwrap());
    }
    FixedOffset::from_str(offset_str).map_err(|_| {
        DatetimeError::InvalidTimezoneOffset {
            offset: offset_str.to_string(),
        }
        .into_vm_error()
    })
}

pub(super) fn format_iso(dt: &DateTime<Utc>) -> String {
    let nanos = dt.timestamp_subsec_nanos();
    if nanos > 0 {
        let base = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
        let frac = format!(".{:09}", nanos);
        let frac = frac.trim_end_matches('0');
        format!("{}{}Z", base, frac)
    } else {
        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

pub(super) fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}

pub(super) fn n(value: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value))
}

pub(super) fn b(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}
