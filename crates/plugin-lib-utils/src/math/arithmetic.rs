use handl_platform::{EvalContext, EvalError, Function};

use super::helpers::{num, require_number, require_two_numbers};
use crate::shared::find_extremum_by;

#[derive(Clone, Copy)]
pub struct Add;
impl Function for Add {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        Ok(num(a + b))
    }
}

#[derive(Clone, Copy)]
pub struct Sub;
impl Function for Sub {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        Ok(num(a - b))
    }
}

#[derive(Clone, Copy)]
pub struct Mul;
impl Function for Mul {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        Ok(num(a * b))
    }
}

#[derive(Clone, Copy)]
pub struct Div;
impl Function for Div {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        if b == 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "non-zero divisor", "zero",)));
        }
        Ok(num(a / b))
    }
}

#[derive(Clone, Copy)]
pub struct Min;
impl Function for Min {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        let values = [a, b];
        let best = find_extremum_by(&values, |x, y| !(*y < *x)).unwrap();
        Ok(num(*best))
    }
}

#[derive(Clone, Copy)]
pub struct Max;
impl Function for Max {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        let values = [a, b];
        let best = find_extremum_by(&values, |x, y| !(*y > *x)).unwrap();
        Ok(num(*best))
    }
}

#[derive(Clone, Copy)]
pub struct Clamp;
impl Function for Clamp {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 3, args.len(),)));
        }
        let value = require_number(&args[0])?;
        let min = require_number(&args[1])?;
        let max = require_number(&args[2])?;
        Ok(num(value.clamp(min, max)))
    }
}

#[derive(Clone, Copy)]
pub struct Mod;
impl Function for Mod {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        Ok(num(a.rem_euclid(b)))
    }
}

#[derive(Clone, Copy)]
pub struct FloorDiv;
impl Function for FloorDiv {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        if b == 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "non-zero divisor", "zero",)));
        }
        Ok(num((a / b).floor()))
    }
}

fn gcd_u64(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd_u64(b, a % b) }
}

#[derive(Clone, Copy)]
pub struct Gcd;
impl Function for Gcd {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        if a < 0.0 || b < 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "non-negative number", "negative number",)));
        }
        Ok(num(gcd_u64(a as u64, b as u64) as f64))
    }
}

#[derive(Clone, Copy)]
pub struct Lcm;
impl Function for Lcm {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        if a < 0.0 || b < 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "non-negative number", "negative number",)));
        }
        let a_u = a as u64;
        let b_u = b as u64;
        if a_u == 0 || b_u == 0 {
            return Ok(num(0.0));
        }
        Ok(num((a_u / gcd_u64(a_u, b_u) * b_u) as f64))
    }
}

#[derive(Clone, Copy)]
pub struct Factorial;
impl Function for Factorial {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        let x = require_number(arg)?;
        if x < 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "non-negative number", "negative number",)));
        }
        if x.fract() != 0.0 {
            return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "integer", "non-integer",)));
        }
        let n = x as u64;
        let mut result: f64 = 1.0;
        for i in 2..=n {
            result *= i as f64;
        }
        Ok(num(result))
    }
}

#[derive(Clone, Copy)]
pub struct Lerp;
impl Function for Lerp {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 3, args.len(),)));
        }
        let start = require_number(&args[0])?;
        let end = require_number(&args[1])?;
        let t = require_number(&args[2])?;
        Ok(num(start + (end - start) * t))
    }
}

#[derive(Clone, Copy)]
pub struct MapRange;
impl Function for MapRange {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 5 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 5, args.len(),)));
        }
        let value = require_number(&args[0])?;
        let in_min = require_number(&args[1])?;
        let in_max = require_number(&args[2])?;
        let out_min = require_number(&args[3])?;
        let out_max = require_number(&args[4])?;
        Ok(num(out_min
            + (value - in_min) * (out_max - out_min)
                / (in_max - in_min)))
    }
}
