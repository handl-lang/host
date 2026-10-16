use handl_platform::{EvalContext, EvalError, Function};

use super::common::{arr, require_one_array};

/// `collections.*.new() → collection`
///
/// Returns a new empty collection.
#[derive(Clone, Copy)]
pub struct New;

impl Function for New {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(arr(&[]))
    }
}

/// `collections.*.from(array: array) → collection`
///
/// Creates a collection from an existing array.
#[derive(Clone, Copy)]
pub struct From;

impl Function for From {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(arr(input))
    }
}

/// `collections.*.to_array(collection: collection) → array`
///
/// Returns the collection as a plain array.
#[derive(Clone, Copy)]
pub struct ToArray;

impl Function for ToArray {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(arr(input))
    }
}
