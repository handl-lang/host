//! Rust `Function` impls for the `math` module — the handls (handlers)
//! for the `op`/`handl` declarations in `math.hnl`.
//!
//! All 54 functions are FFI atoms: they call `f64` methods or Rust
//! arithmetic that cannot be expressed as pure handl arrows without
//! losing the exact `TypeMismatch` error messages the tests rely on.
//! The `op`/`handl` declaration form (ADR-0077 Phase 2) declares the
//! interface in `math.hnl`; these impls provide the logic and are
//! registered in an [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use handl_platform::{EvalContext, EvalError, Function};

use crate::shared::{b, find_extremum_by, num};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_number(arg: &serde_json::Value) -> Result<f64, EvalError> {
    arg.as_f64()
        .ok_or(handl_platform::EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))
}

fn require_two_numbers(args: &[serde_json::Value]) -> Result<(f64, f64), EvalError> {
    if args.len() < 2 {
        return Err(handl_platform::EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let a = require_number(&args[0])?;
    let b = require_number(&args[1])?;
    Ok((a, b))
}

macro_rules! unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let arg = args
                    .first()
                    .ok_or(handl_platform::EvalError::TypeMismatch(format!(
                        "expected {} args, got {}",
                        1, 0,
                    )))?;
                let x = require_number(arg)?;
                Ok(num($body(x)))
            }
        }
    };
}

#[allow(unused_imports)]
use unary_fn;

macro_rules! bool_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let arg = args
                    .first()
                    .ok_or(handl_platform::EvalError::TypeMismatch(format!(
                        "expected {} args, got {}",
                        1, 0,
                    )))?;
                let x = require_number(arg)?;
                Ok(b($body(x)))
            }
        }
    };
}

#[allow(unused_imports)]
use bool_unary_fn;

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

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
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-zero divisor", "zero",
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-zero divisor", "zero",
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
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
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        let x = require_number(arg)?;
        if x < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        if x.fract() != 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "integer", "non-integer",
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
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
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                5,
                args.len(),
            )));
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

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Pi;
impl Function for Pi {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::PI))
    }
}

#[derive(Clone, Copy)]
pub struct E;
impl Function for E {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::E))
    }
}

#[derive(Clone, Copy)]
pub struct Nan;
impl Function for Nan {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(f64::NAN))
    }
}

#[derive(Clone, Copy)]
pub struct Infinity;
impl Function for Infinity {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(f64::INFINITY))
    }
}

#[derive(Clone, Copy)]
pub struct Tau;
impl Function for Tau {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(num(std::f64::consts::TAU))
    }
}

// ---------------------------------------------------------------------------
// Exponential / logarithmic
// ---------------------------------------------------------------------------

unary_fn!(Sqrt, |x: f64| x.sqrt());
unary_fn!(Log, |x: f64| x.ln());
unary_fn!(Log10, |x: f64| x.log10());
unary_fn!(Exp, |x: f64| x.exp());
unary_fn!(Log2, |x: f64| x.log2());
unary_fn!(Cbrt, |x: f64| x.cbrt());

#[derive(Clone, Copy)]
pub struct Pow;
impl Function for Pow {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (base, exp) = require_two_numbers(args)?;
        Ok(num(base.powf(exp)))
    }
}

// ---------------------------------------------------------------------------
// Rounding
// ---------------------------------------------------------------------------

unary_fn!(Abs, |x: f64| x.abs());
unary_fn!(Floor, |x: f64| x.floor());
unary_fn!(Ceil, |x: f64| x.ceil());
unary_fn!(Round, |x: f64| x.round());
unary_fn!(Sign, |x: f64| x.signum());
unary_fn!(Trunc, |x: f64| x.trunc());
unary_fn!(Fract, |x: f64| x.fract());

#[derive(Clone, Copy)]
pub struct RoundTo;
impl Function for RoundTo {
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
        let value = require_number(&args[0])?;
        let decimals = require_number(&args[1])?;
        if decimals < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let multiplier = 10f64.powi(decimals as i32);
        Ok(num((value * multiplier).round() / multiplier))
    }
}

// ---------------------------------------------------------------------------
// Trigonometry
// ---------------------------------------------------------------------------

unary_fn!(Sin, |x: f64| x.sin());
unary_fn!(Cos, |x: f64| x.cos());
unary_fn!(Tan, |x: f64| x.tan());
unary_fn!(Asin, |x: f64| x.asin());
unary_fn!(Acos, |x: f64| x.acos());
unary_fn!(Atan, |x: f64| x.atan());
unary_fn!(Sinh, |x: f64| x.sinh());
unary_fn!(Cosh, |x: f64| x.cosh());
unary_fn!(Tanh, |x: f64| x.tanh());
unary_fn!(Asinh, |x: f64| x.asinh());
unary_fn!(Acosh, |x: f64| x.acosh());
unary_fn!(Atanh, |x: f64| x.atanh());
unary_fn!(Degrees, |x: f64| x.to_degrees());
unary_fn!(Radians, |x: f64| x.to_radians());

#[derive(Clone, Copy)]
pub struct Atan2;
impl Function for Atan2 {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (y, x) = require_two_numbers(args)?;
        Ok(num(y.atan2(x)))
    }
}

#[derive(Clone, Copy)]
pub struct Hypot;
impl Function for Hypot {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_numbers(args)?;
        Ok(num(a.hypot(b)))
    }
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

bool_unary_fn!(IsNan, |x: f64| x.is_nan());
bool_unary_fn!(IsFinite, |x: f64| x.is_finite());
bool_unary_fn!(IsInfinite, |x: f64| x.is_infinite());
bool_unary_fn!(IsInteger, |x: f64| x.is_finite() && x.fract() == 0.0);

use std::sync::Arc;
use handl_plugin_lib_handl::OpRegistry;

pub const MATH_OPS: &[&str] = &[
    "math.abs",
    "math.add",
    "math.ceil",
    "math.clamp",
    "math.cos",
    "math.cosh",
    "math.cbrt",
    "math.asin",
    "math.acos",
    "math.atan",
    "math.atan2",
    "math.asinh",
    "math.acosh",
    "math.atanh",
    "math.degrees",
    "math.div",
    "math.e",
    "math.exp",
    "math.factorial",
    "math.floor",
    "math.floor_div",
    "math.fract",
    "math.gcd",
    "math.hypot",
    "math.infinity",
    "math.is_finite",
    "math.is_infinite",
    "math.is_integer",
    "math.is_nan",
    "math.lcm",
    "math.lerp",
    "math.log",
    "math.log10",
    "math.log2",
    "math.map_range",
    "math.max",
    "math.min",
    "math.mod",
    "math.mul",
    "math.nan",
    "math.pi",
    "math.pow",
    "math.radians",
    "math.round",
    "math.round_to",
    "math.sign",
    "math.sin",
    "math.sinh",
    "math.sqrt",
    "math.sub",
    "math.tan",
    "math.tanh",
    "math.tau",
    "math.trunc",
];

pub fn math_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("math.abs", Arc::new(Abs));
    registry.register("math.add", Arc::new(Add));
    registry.register("math.ceil", Arc::new(Ceil));
    registry.register("math.clamp", Arc::new(Clamp));
    registry.register("math.cos", Arc::new(Cos));
    registry.register("math.cosh", Arc::new(Cosh));
    registry.register("math.cbrt", Arc::new(Cbrt));
    registry.register("math.asin", Arc::new(Asin));
    registry.register("math.acos", Arc::new(Acos));
    registry.register("math.atan", Arc::new(Atan));
    registry.register("math.atan2", Arc::new(Atan2));
    registry.register("math.asinh", Arc::new(Asinh));
    registry.register("math.acosh", Arc::new(Acosh));
    registry.register("math.atanh", Arc::new(Atanh));
    registry.register("math.degrees", Arc::new(Degrees));
    registry.register("math.div", Arc::new(Div));
    registry.register("math.e", Arc::new(E));
    registry.register("math.exp", Arc::new(Exp));
    registry.register("math.factorial", Arc::new(Factorial));
    registry.register("math.floor", Arc::new(Floor));
    registry.register("math.floor_div", Arc::new(FloorDiv));
    registry.register("math.fract", Arc::new(Fract));
    registry.register("math.gcd", Arc::new(Gcd));
    registry.register("math.hypot", Arc::new(Hypot));
    registry.register("math.infinity", Arc::new(Infinity));
    registry.register("math.is_finite", Arc::new(IsFinite));
    registry.register("math.is_infinite", Arc::new(IsInfinite));
    registry.register("math.is_integer", Arc::new(IsInteger));
    registry.register("math.is_nan", Arc::new(IsNan));
    registry.register("math.lcm", Arc::new(Lcm));
    registry.register("math.lerp", Arc::new(Lerp));
    registry.register("math.log", Arc::new(Log));
    registry.register("math.log10", Arc::new(Log10));
    registry.register("math.log2", Arc::new(Log2));
    registry.register("math.map_range", Arc::new(MapRange));
    registry.register("math.max", Arc::new(Max));
    registry.register("math.min", Arc::new(Min));
    registry.register("math.mod", Arc::new(Mod));
    registry.register("math.mul", Arc::new(Mul));
    registry.register("math.nan", Arc::new(Nan));
    registry.register("math.pi", Arc::new(Pi));
    registry.register("math.pow", Arc::new(Pow));
    registry.register("math.radians", Arc::new(Radians));
    registry.register("math.round", Arc::new(Round));
    registry.register("math.round_to", Arc::new(RoundTo));
    registry.register("math.sign", Arc::new(Sign));
    registry.register("math.sin", Arc::new(Sin));
    registry.register("math.sinh", Arc::new(Sinh));
    registry.register("math.sqrt", Arc::new(Sqrt));
    registry.register("math.sub", Arc::new(Sub));
    registry.register("math.tan", Arc::new(Tan));
    registry.register("math.tanh", Arc::new(Tanh));
    registry.register("math.tau", Arc::new(Tau));
    registry.register("math.trunc", Arc::new(Trunc));
    registry
}
