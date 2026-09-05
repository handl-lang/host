use std::sync::{Arc, OnceLock};
use std::time::Instant;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value;
use serde_json::json;

use crate::framework::error::TestError;
use crate::framework::shared::{call_function, obj, require_function, require_integer};
use crate::framework::{test_op_registry, TEST_HNL};

/// The op names declared in `test.hnl` for the `benchmark` module.
const BENCHMARK_OPS: &[&str] = &["benchmark.run"];

/// A thread-safe lazy-initialized [`HandlModule`] for benchmark.
static BENCHMARK_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for benchmark.
fn benchmark_module() -> &'static HandlModule {
    BENCHMARK_MODULE.get_or_init(|| {
        let registry = Arc::new(test_op_registry());
        HandlModule::new("benchmark", TEST_HNL, &[]).with_ops(BENCHMARK_OPS, registry)
    })
}

/// The `benchmark` module — simple wall-clock timing for functions.
///
/// Functions: `benchmark.run`.
#[derive(Clone, Copy)]
pub struct BenchmarkModule;

impl Module for BenchmarkModule {
    fn name(&self) -> &str {
        benchmark_module().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        benchmark_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        benchmark_module().functions()
    }
}

#[derive(Clone, Copy)]
pub struct Run;
impl Function for Run {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() < 2 {
            return Err(TestError::ArgumentCount {
                expected: 2,
                got: args.len(),
            }
            .into());
        }

        let func = require_function(&args[0])?;
        let iterations = require_integer(&args[1])?;
        if iterations <= 0 {
            return Err(TestError::AssertionFailed {
                message: "iterations must be positive".to_string(),
            }
            .into());
        }

        let bench_args: Vec<Value> = if args.len() >= 3 {
            match args[2].as_array() {
                Some(arr) => arr.iter().map(|v| json!(v.clone())).collect(),
                None => {
                    return Err(TestError::TypeMismatch {
                        expected: "array",
                        got: crate::framework::shared::type_name(&args[2]),
                    }
                    .into());
                }
            }
        } else {
            Vec::new()
        };

        let start = Instant::now();
        let mut last_result = None;
        for _ in 0..iterations {
            last_result = Some(call_function(ctx, &func, &bench_args)?);
        }
        let total = start.elapsed().as_millis() as f64;

        Ok(obj(&[
            ("iterations", serde_json::Value::from(iterations)),
            ("total_ms", serde_json::Value::from(total)),
            (
                "mean_ms",
                serde_json::Value::from(total / iterations as f64),
            ),
            (
                "result",
                last_result.map(|v| v).unwrap_or(serde_json::Value::Null),
            ),
        ]))
    }
}
