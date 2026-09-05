use std::sync::{Arc, OnceLock};

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::Value;
use serde_json::json;

use crate::error::TestError;
use crate::shared::{call_function, require_function, require_integer};
use crate::{test_op_registry, TEST_HNL};

/// The op names declared in `test.hnl` for the `fuzz` module.
const FUZZ_OPS: &[&str] = &["fuzz.run"];

/// A thread-safe lazy-initialized [`HandlModule`] for fuzz.
static FUZZ_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for fuzz.
fn fuzz_module() -> &'static HandlModule {
    FUZZ_MODULE.get_or_init(|| {
        let registry = Arc::new(test_op_registry());
        HandlModule::new("fuzz", TEST_HNL, &[]).with_ops(FUZZ_OPS, registry)
    })
}

/// The `fuzz` module — structure-agnostic random-input stress testing.
///
/// Functions: `fuzz.run`.
#[derive(Clone, Copy)]
pub struct FuzzModule;

impl Module for FuzzModule {
    fn name(&self) -> &str {
        fuzz_module().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        fuzz_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        fuzz_module().functions()
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
        let count = require_integer(&args[1])?;
        if count <= 0 {
            return Err(TestError::AssertionFailed {
                message: "count must be positive".to_string(),
            }
            .into());
        }

        let seed: u64 = if args.len() >= 3 {
            require_integer(&args[2])? as u64
        } else {
            let mut system_rng = rand::rng();
            system_rng.random::<u64>()
        };

        let mut rng = StdRng::seed_from_u64(seed);
        let mut failures = 0usize;
        let mut first_error = None;

        for _ in 0..count {
            let inputs: Vec<Value> = (0..func.params.len())
                .map(|_| json!(rng.random::<f64>()))
                .collect();

            if let Err(e) = call_function(ctx, &func, &inputs) {
                failures += 1;
                if first_error.is_none() {
                    first_error = Some(e.to_string());
                }
            }
        }

        let mut report = serde_json::Map::new();
        report.insert("runs".to_string(), serde_json::Value::from(count));
        report.insert("failures".to_string(), serde_json::Value::from(failures));
        if let Some(msg) = first_error {
            report.insert("first_error".to_string(), serde_json::Value::String(msg));
        }

        Ok(json!(serde_json::Value::Object(report)))
    }
}
