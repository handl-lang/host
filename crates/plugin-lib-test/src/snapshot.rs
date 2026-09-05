use std::sync::{Arc, OnceLock};

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value;

use crate::error::TestError;
use crate::shared::{b, require_len, require_string, str};
use crate::{test_op_registry, TEST_HNL};

/// The op names declared in `test.hnl` for the `snapshot` module.
const SNAPSHOT_OPS: &[&str] = &["snapshot.format", "snapshot.match"];

/// A thread-safe lazy-initialized [`HandlModule`] for snapshot.
static SNAPSHOT_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for snapshot.
fn snapshot_module() -> &'static HandlModule {
    SNAPSHOT_MODULE.get_or_init(|| {
        let registry = Arc::new(test_op_registry());
        HandlModule::new("snapshot", TEST_HNL, &[]).with_ops(SNAPSHOT_OPS, registry)
    })
}

/// The `snapshot` module — snapshot formatting and matching.
///
/// Functions: `snapshot.format`, `snapshot.match`.
#[derive(Clone, Copy)]
pub struct SnapshotModule;

impl Module for SnapshotModule {
    fn name(&self) -> &str {
        snapshot_module().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        snapshot_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        snapshot_module().functions()
    }
}

#[derive(Clone, Copy)]
pub struct Format;
impl Function for Format {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 1)?;
        let serialized =
            serde_json::to_string_pretty(&args[0]).unwrap_or_else(|_| "null".to_string());
        Ok(str(serialized))
    }
}

#[derive(Clone, Copy)]
pub struct Match;
impl Function for Match {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 3)?;
        let name = require_string(&args[0])?;
        let actual = &args[1];
        let expected = &args[2];

        if actual != expected {
            return Err(TestError::SnapshotMismatch {
                name: name.to_string(),
                actual: actual.clone(),
                expected: expected.clone(),
            }
            .into());
        }

        Ok(b(true))
    }
}
