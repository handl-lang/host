use handl_platform::{EvalContext, EvalError, Function, Module};
use serde_json::Value;
use serde_json::json;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::error::TestError;
use crate::shared::{b, call_function, require_function, require_integer};

/// The `property` module — property-based testing with seeded RNG.
///
/// Functions: `property.for_all`.
#[derive(Clone, Copy)]
pub struct PropertyModule;

impl Module for PropertyModule {
    fn name(&self) -> &str {
        "property"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "for_all" => Some(&ForAll),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct ForAll;
impl Function for ForAll {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
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

        for _ in 0..count {
            let inputs: Vec<Value> = (0..func.params.len())
                .map(|_| json!(rng.random::<f64>()))
                .collect();

            match call_function(ctx, &func, &inputs) {
                Ok(result) => {
                    if !result.as_bool().unwrap_or(false) {
                        let counterexample: Vec<_> =
                            inputs.iter().map(|v| v.clone()).collect();
                        return Err(TestError::PropertyFailed {
                            counterexample: serde_json::Value::Array(counterexample),
                            message: "property returned false".to_string(),
                        }
                        .into());
                    }
                }
                Err(e) => {
                    let counterexample: Vec<_> = inputs.iter().map(|v| v.clone()).collect();
                    return Err(TestError::PropertyFailed {
                        counterexample: serde_json::Value::Array(counterexample),
                        message: e.to_string(),
                    }
                    .into());
                }
            }
        }

        Ok(b(true))
    }
}
