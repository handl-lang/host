//! The `math` module — numeric utility functions.
//!
//! Provides functions callable as `math.abs(x)`, `math.floor(x)`, etc.
//! in handl expressions.
//!
//! The math *interface* is declared in `handl-packages/utils/math.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 54
//! functions are FFI atoms — they call `f64` methods or Rust arithmetic
//! that cannot be expressed as pure handl arrows without losing the
//! exact `TypeMismatch` error messages the tests rely on. The Rust
//! `Function` impls in [`ops`] are registered as handls (handlers) in an
//! [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].

mod ops;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{
    Abs, Acos, Acosh, Add, Asin, Asinh, Atan, Atan2, Atanh, Cbrt, Ceil, Clamp, Cos, Cosh, Degrees,
    Div, E, Exp, Factorial, Floor, FloorDiv, Fract, Gcd, Hypot, Infinity, IsFinite, IsInfinite,
    IsInteger, IsNan, Lcm, Lerp, Log, Log10, Log2, MapRange, Max, Min, Mod, Mul, Nan, Pi, Pow,
    Radians, Round, RoundTo, Sign, Sin, Sinh, Sqrt, Sub, Tan, Tanh, Tau, Trunc,
};

/// The handl source declaring the math `op` interface — the single
/// source of truth for the math API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// `math.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 54 math functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const MATH_HNL: &str = include_str!("../../../../handl-packages/utils/math.hnl");

/// The op names declared in `math.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const MATH_OPS: &[&str] = &[
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

/// Build the [`OpRegistry`] for math, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"math.add"`) maps to its Rust `Function` impl
/// (e.g. [`Add`]). No runtime guarantee checkers are attached — the math
/// atoms are trusted.
fn math_op_registry() -> OpRegistry {
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

/// Build the [`HandlModule`] for math.
///
/// All 54 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_math_module() -> HandlModule {
    let registry = Arc::new(math_op_registry());
    HandlModule::new("math", MATH_HNL, &[]).with_ops(MATH_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for math.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static MATH_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for math, initializing it on first call.
fn math_module() -> &'static HandlModule {
    MATH_MODULE.get_or_init(build_math_module)
}

/// The `math` module — numeric utility functions.
///
/// Provides `math.abs`, `math.floor`, `math.ceil`, `math.round`,
/// `math.min`, `math.max`, `math.sqrt`, `math.pow`, `math.sign`,
/// `math.trunc`, `math.clamp`, `math.log`, `math.log10`, `math.log2`,
/// `math.exp`, `math.sin`, `math.cos`, `math.tan`, `math.asin`,
/// `math.acos`, `math.atan`, `math.atan2`, `math.hypot`, `math.mod`,
/// `math.cbrt`, `math.degrees`, `math.radians`, `math.round_to`,
/// `math.pi`, `math.e`, and more. The interface is declared in
/// `math.hnl` via `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct MathModule;

impl Module for MathModule {
    fn name(&self) -> &str {
        "math"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        math_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        math_module().functions()
    }
}

#[cfg(test)]
mod arithmetic_tests;

#[cfg(test)]
mod math_tests;
