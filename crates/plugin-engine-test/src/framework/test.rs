use std::sync::{Arc, OnceLock};

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_kit_canonical::handl_module::HandlModule;
use serde_json::Value;
use serde_json::json;

use crate::framework::error::TestError;
use crate::framework::shared::{call_function, obj, require_array, require_function};
use crate::framework::{test_op_registry, TEST_HNL};

/// The op names declared in `test.hnl` for the `test` module.
const TEST_OPS: &[&str] = &["test.run"];

/// A thread-safe lazy-initialized [`HandlModule`] for test.
static TEST_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for test.
fn test_module() -> &'static HandlModule {
    TEST_MODULE.get_or_init(|| {
        let registry = Arc::new(test_op_registry());
        HandlModule::new("test", TEST_HNL, &[]).with_ops(TEST_OPS, registry)
    })
}

/// The `test` module — test suite execution and reporting.
///
/// Functions: `test.run`.
#[derive(Clone, Copy)]
pub struct TestModule;

impl Module for TestModule {
    fn name(&self) -> &str {
        test_module().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        test_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        test_module().functions()
    }
}

#[derive(Clone, Copy)]
pub struct Run;
impl Function for Run {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.is_empty() {
            return Err(TestError::ArgumentCount {
                expected: 1,
                got: 0,
            }
            .into());
        }

        let suite = require_array(&args[0])?;
        let mut results = Vec::new();
        let mut passed = 0usize;
        let mut failed = 0usize;

        for (i, case) in suite.iter().enumerate() {
            let mut result_map = serde_json::Map::new();
            let name = case
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("case {i}"))
                .to_string();

            result_map.insert("name".to_string(), serde_json::Value::String(name));

            let body = match case.get("body") {
                Some(body_val) => json!(body_val.clone()),
                None => json!(serde_json::Value::Null),
            };

            let func = match require_function(&body) {
                Ok(f) => f,
                Err(e) => {
                    result_map.insert("pass".to_string(), serde_json::Value::Bool(false));
                    result_map.insert(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    );
                    failed += 1;
                    results.push(serde_json::Value::Object(result_map));
                    continue;
                }
            };

            let test_args: Vec<Value> = match case.get("args") {
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().map(|v| json!(v.clone())).collect()
                }
                Some(_) => {
                    result_map.insert("pass".to_string(), serde_json::Value::Bool(false));
                    result_map.insert(
                        "error".to_string(),
                        serde_json::Value::String("args must be an array".to_string()),
                    );
                    failed += 1;
                    results.push(serde_json::Value::Object(result_map));
                    continue;
                }
                None => Vec::new(),
            };

            match call_function(ctx, &func, &test_args) {
                Ok(_) => {
                    result_map.insert("pass".to_string(), serde_json::Value::Bool(true));
                    passed += 1;
                }
                Err(e) => {
                    result_map.insert("pass".to_string(), serde_json::Value::Bool(false));
                    result_map.insert(
                        "error".to_string(),
                        serde_json::Value::String(e.to_string()),
                    );
                    failed += 1;
                }
            }

            results.push(serde_json::Value::Object(result_map));
        }

        Ok(obj(&[
            ("total", serde_json::Value::from(passed + failed)),
            ("passed", serde_json::Value::from(passed)),
            ("failed", serde_json::Value::from(failed)),
            ("results", serde_json::Value::Array(results)),
        ]))
    }
}
