use handl_platform::{EvalContext, EvalError, Function};

use super::common::{arr, b, num, require_array, require_one_array};
use super::error::CollectionsError;

/// Which end of an array-backed collection to operate on.
#[derive(Clone, Copy)]
pub enum End {
    Front,
    Back,
}

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

/// `collections.*.len(collection: collection) → number`
#[derive(Clone, Copy)]
pub struct Len;

impl Function for Len {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(num(input.len() as f64))
    }
}

/// `collections.*.is_empty(collection: collection) → bool`
#[derive(Clone, Copy)]
pub struct IsEmpty;

impl Function for IsEmpty {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(b(input.is_empty()))
    }
}

/// `collections.*.push(collection: collection, value: value) → collection`
///
/// Pushes a value onto the chosen end of the collection.
#[derive(Clone, Copy)]
pub struct Push {
    pub end: End,
}

impl Function for Push {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_array(args, 0)?;
        let value = args.get(1).ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)))?;
        let mut values = input.to_vec();
        match self.end {
            End::Back => values.push(value.clone()),
            End::Front => values.insert(0, value.clone()),
        }
        Ok(arr(&values))
    }
}

/// `collections.*.pop(collection: collection) → collection`
///
/// Removes the value at the chosen end and returns the shorter collection.
#[derive(Clone, Copy)]
pub struct Pop {
    pub end: End,
}

impl Function for Pop {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        if input.is_empty() {
            return Err(CollectionsError::Empty.into());
        }
        let values = match self.end {
            End::Back => input[..input.len() - 1].to_vec(),
            End::Front => input[1..].to_vec(),
        };
        Ok(arr(&values))
    }
}

/// `collections.*.peek(collection: collection) → value`
///
/// Returns the value at the chosen end without removing it.
#[derive(Clone, Copy)]
pub struct Peek {
    pub end: End,
}

impl Function for Peek {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        if input.is_empty() {
            return Err(CollectionsError::Empty.into());
        }
        let value = match self.end {
            End::Back => input.last().unwrap(),
            End::Front => input.first().unwrap(),
        };
        Ok(serde_json::Value::from(value.clone()))
    }
}
