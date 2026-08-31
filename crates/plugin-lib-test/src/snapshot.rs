use handl_platform::{EvalContext, EvalError, Function, Module};
use serde_json::Value;

use crate::error::TestError;
use crate::shared::{b, require_len, require_string, str};

/// The `snapshot` module — snapshot formatting and matching.
///
/// Functions: `snapshot.format`, `snapshot.match`.
#[derive(Clone, Copy)]
pub struct SnapshotModule;

impl Module for SnapshotModule {
    fn name(&self) -> &str {
        "snapshot"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "format" => Some(&Format),
            "match" => Some(&Match),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Format;
impl Function for Format {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        require_len(args, 1)?;
        let serialized =
            serde_json::to_string_pretty(&args[0]).unwrap_or_else(|_| "null".to_string());
        Ok(str(serialized))
    }
}

#[derive(Clone, Copy)]
struct Match;
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
