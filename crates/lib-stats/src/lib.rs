//! # handl-lib-stats
//!
//! Statistical aggregate functions for the handl runtime.

use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value;

fn num(value: f64) -> NonFunctionValue {
    NonFunctionValue::new(serde_json::json!(value))
}

fn require_one_array(args: &[NonFunctionValue]) -> Result<&[Value], Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: 0,
    })?;
    arg.as_array().ok_or(Error::TypeMismatch {
        expected: "array",
        got: "non-array",
    })
}

macro_rules! stats_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl Function<NonFunctionValue> for $struct_name {
            fn call(
                &self,
                args: &[NonFunctionValue],
                _ctx: &mut dyn Context<NonFunctionValue>,
            ) -> Result<NonFunctionValue, Error> {
                let input = require_one_array(args)?;
                Ok($body(input))
            }
        }
    };
}

stats_unary_fn!(Sum, |input: &[Value]| {
    let total: f64 = input.iter().filter_map(|v| v.as_f64()).sum();
    num(total)
});

stats_unary_fn!(Avg, |input: &[Value]| {
    let numbers: Vec<f64> = input.iter().filter_map(|v| v.as_f64()).collect();
    if numbers.is_empty() {
        num(0.0)
    } else {
        num(numbers.iter().sum::<f64>() / numbers.len() as f64)
    }
});

stats_unary_fn!(Product, |input: &[Value]| {
    let product: f64 = input.iter().filter_map(|v| v.as_f64()).product();
    num(product)
});

#[derive(Clone, Copy)]
pub struct Min;
impl Function<NonFunctionValue> for Min {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_array(args)?;
        let numbers: Vec<f64> = input.iter().filter_map(|v| v.as_f64()).collect();
        match numbers.iter().copied().min_by(|a, c| a.total_cmp(c)) {
            Some(m) => Ok(num(m)),
            None => Ok(NonFunctionValue::new(Value::Null)),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Max;
impl Function<NonFunctionValue> for Max {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_array(args)?;
        let numbers: Vec<f64> = input.iter().filter_map(|v| v.as_f64()).collect();
        match numbers.iter().copied().max_by(|a, c| a.total_cmp(c)) {
            Some(m) => Ok(num(m)),
            None => Ok(NonFunctionValue::new(Value::Null)),
        }
    }
}

stats_unary_fn!(Median, |input: &[Value]| {
    let mut numbers: Vec<f64> = input.iter().filter_map(|v| v.as_f64()).collect();
    if numbers.is_empty() {
        return num(0.0);
    }
    numbers.sort_by(|a, b| a.total_cmp(b));
    let len = numbers.len();
    if len % 2 == 0 {
        num((numbers[len / 2 - 1] + numbers[len / 2]) / 2.0)
    } else {
        num(numbers[len / 2])
    }
});

stats_unary_fn!(Stddev, |input: &[Value]| {
    let numbers: Vec<f64> = input.iter().filter_map(|v| v.as_f64()).collect();
    if numbers.len() < 2 {
        return num(0.0);
    }
    let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
    let variance = numbers.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / numbers.len() as f64;
    num(variance.sqrt())
});

/// The `stats` module — statistical aggregate functions.
///
/// Provides `stats.sum`, `stats.avg`, `stats.median`, `stats.stddev`,
/// `stats.product`, `stats.min`, `stats.max`.
#[derive(Clone, Copy)]
pub struct StatsModule;

impl Module for StatsModule {
    fn name(&self) -> &str {
        "stats"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "sum" => Some(&Sum),
            "avg" => Some(&Avg),
            "product" => Some(&Product),
            "min" => Some(&Min),
            "max" => Some(&Max),
            "median" => Some(&Median),
            "stddev" => Some(&Stddev),
            _ => None,
        }
    }
}
