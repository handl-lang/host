use std::cmp::Ordering;

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value as JsonValue;

/// Total ordering over arbitrary JSON values.
///
/// Ordering across kinds follows `value_kind` (null < bool < number < string
/// < array < object); within a kind the natural ordering is used. This mirrors
/// the semantics of `cmp.compare` so that `prim.compare` is consistent with the
/// rest of the runtime.
fn value_kind(v: &JsonValue) -> u8 {
    match v {
        JsonValue::Null => 0,
        JsonValue::Bool(_) => 1,
        JsonValue::Number(_) => 2,
        JsonValue::String(_) => 3,
        JsonValue::Array(_) => 4,
        JsonValue::Object(_) => 5,
    }
}

fn compare_values(a: &JsonValue, b: &JsonValue) -> Ordering {
    value_kind(a)
        .cmp(&value_kind(b))
        .then_with(|| match (a, b) {
            (JsonValue::Null, JsonValue::Null) => Ordering::Equal,
            (JsonValue::Bool(aa), JsonValue::Bool(bb)) => aa.cmp(bb),
            (JsonValue::Number(aa), JsonValue::Number(bb)) => {
                aa.as_f64().unwrap().total_cmp(&bb.as_f64().unwrap())
            }
            (JsonValue::String(aa), JsonValue::String(bb)) => aa.cmp(bb),
            (JsonValue::Array(aa), JsonValue::Array(bb)) => {
                for (x, y) in aa.iter().zip(bb.iter()) {
                    let c = compare_values(x, y);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                aa.len().cmp(&bb.len())
            }
            (JsonValue::Object(aa), JsonValue::Object(bb)) => {
                if aa.len() != bb.len() {
                    return aa.len().cmp(&bb.len());
                }
                let mut av: Vec<_> = aa.iter().map(|(k, v)| (k.as_str(), v)).collect();
                let mut bv: Vec<_> = bb.iter().map(|(k, v)| (k.as_str(), v)).collect();
                av.sort_by(|x, y| x.0.cmp(y.0));
                bv.sort_by(|x, y| x.0.cmp(y.0));
                for ((ak, avv), (bk, bvv)) in av.iter().zip(bv.iter()) {
                    let c = ak.cmp(bk);
                    if c != Ordering::Equal {
                        return c;
                    }
                    let c = compare_values(avv, bvv);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }
            _ => Ordering::Equal,
        })
}

fn require_two(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    Ok((&args[0], &args[1]))
}

/// `prim.compare(a: Int, b: Int) -> Int`
///
/// Compares two values and returns -1, 0, or 1 (like `cmp.compare`).
#[derive(Clone, Copy)]
pub struct Compare;

impl Function for Compare {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(serde_json::Value::from(match compare_values(a, b_arg) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }))
    }
}

/// `prim.or(a: Int, b: Int) -> Int`
///
/// Bitwise OR on two integers. Used for composing POSIX flag constants
/// (e.g. `prim.or(O_RDONLY, O_CLOEXEC)`). Both arguments must be integers;
/// otherwise the call errors.
#[derive(Clone, Copy)]
pub struct Or;

impl Function for Or {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        let av = a.as_i64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "integer", "non-integer",
        )))?;
        let bv = b_arg.as_i64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "integer", "non-integer",
        )))?;
        Ok(serde_json::Value::from(av | bv))
    }
}
