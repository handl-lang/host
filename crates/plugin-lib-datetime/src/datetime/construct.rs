
use super::common::{format_iso, s};
use crate::DatetimeError;
use chrono::{TimeZone, Utc};
use handl_platform::{EvalContext, EvalError, Function};

fn get_number(args: &[serde_json::Value], index: usize, default: f64) -> Result<f64, EvalError> {
    match args.get(index) {
        Some(v) => v.as_f64().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "number", "non-number",))),
        None => Ok(default),
    }
}

#[derive(Clone, Copy)]
pub struct FromComponents;
impl Function for FromComponents {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 3, args.len(),)));
        }
        let year = get_number(args, 0, 0.0)? as i32;
        let month = get_number(args, 1, 0.0)? as u32;
        let day = get_number(args, 2, 0.0)? as u32;
        let hour = get_number(args, 3, 0.0)? as u32;
        let minute = get_number(args, 4, 0.0)? as u32;
        let second = get_number(args, 5, 0.0)? as u32;

        let dt = Utc
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .ok_or(
                DatetimeError::InvalidDateComponents {
                    year,
                    month,
                    day,
                    hour,
                    minute,
                    second,
                }
                .into_vm_error(),
            )?;

        Ok(s(format_iso(&dt)))
    }
}
