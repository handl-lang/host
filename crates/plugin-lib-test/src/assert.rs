use std::sync::{Arc, OnceLock};

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value;

use crate::error::TestError;
use crate::shared::{b, call_function, require_bool, require_function, require_len};
use crate::{test_op_registry, TEST_HNL};

/// The op names declared in `test.hnl` for the `assert` module.
const ASSERT_OPS: &[&str] = &[
    "assert.equal",
    "assert.not_equal",
    "assert.true",
    "assert.false",
    "assert.contains",
    "assert.throws",
];

/// A thread-safe lazy-initialized [`HandlModule`] for assert.
static ASSERT_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for assert.
fn assert_module() -> &'static HandlModule {
    ASSERT_MODULE.get_or_init(|| {
        let registry = Arc::new(test_op_registry());
        HandlModule::new("assert", TEST_HNL, &[]).with_ops(ASSERT_OPS, registry)
    })
}

/// The `assert` module — assertion primitives for test expressions.
///
/// Functions: `assert.equal`, `assert.not_equal`, `assert.true`,
/// `assert.false`, `assert.contains`, `assert.throws`.
#[derive(Clone, Copy)]
pub struct AssertModule;

impl Module for AssertModule {
    fn name(&self) -> &str {
        assert_module().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        assert_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        assert_module().functions()
    }
}

#[derive(Clone, Copy)]
pub struct Equal;
impl Function for Equal {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 2)?;
        if args[0] != args[1] {
            return Err(TestError::AssertionFailed {
                message: format!(
                    "expected {} to equal {}",
                    serde_json::to_string(&args[0]).unwrap_or_default(),
                    serde_json::to_string(&args[1]).unwrap_or_default()
                ),
            }
            .into());
        }
        Ok(b(true))
    }
}

#[derive(Clone, Copy)]
pub struct NotEqual;
impl Function for NotEqual {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 2)?;
        if args[0] == args[1] {
            return Err(TestError::AssertionFailed {
                message: format!(
                    "expected {} to not equal {}",
                    serde_json::to_string(&args[0]).unwrap_or_default(),
                    serde_json::to_string(&args[1]).unwrap_or_default()
                ),
            }
            .into());
        }
        Ok(b(true))
    }
}

#[derive(Clone, Copy)]
pub struct True;
impl Function for True {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 1)?;
        let value = require_bool(&args[0])?;
        if !value {
            return Err(TestError::AssertionFailed {
                message: "expected true".to_string(),
            }
            .into());
        }
        Ok(b(true))
    }
}

#[derive(Clone, Copy)]
pub struct False;
impl Function for False {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 1)?;
        let value = require_bool(&args[0])?;
        if value {
            return Err(TestError::AssertionFailed {
                message: "expected false".to_string(),
            }
            .into());
        }
        Ok(b(true))
    }
}

#[derive(Clone, Copy)]
pub struct Contains;
impl Function for Contains {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 2)?;
        let haystack = &args[0];
        let needle = &args[1];

        let found = match (haystack, needle) {
            (serde_json::Value::Array(arr), _) => arr.contains(needle),
            (serde_json::Value::Object(map), serde_json::Value::String(key)) => {
                map.contains_key(key)
            }
            (serde_json::Value::String(s), serde_json::Value::String(part)) => {
                s.contains(part.as_str())
            }
            _ => false,
        };

        if !found {
            return Err(TestError::AssertionFailed {
                message: format!("expected {haystack} to contain {needle}"),
            }
            .into());
        }
        Ok(b(true))
    }
}

#[derive(Clone, Copy)]
pub struct Throws;
impl Function for Throws {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 1)?;
        let func = require_function(&args[0])?;
        if call_function(ctx, &func, &[]).is_err() {
            return Ok(b(true));
        }
        Err(TestError::AssertionFailed {
            message: "expected function to throw, but it did not".to_string(),
        }
        .into())
    }
}
