//! # FP host functions: `fn.fanout_fn`, `field_access`
//!
//! Host functions for the functional-programming `Foreign` schemes in the
//! canonical eval data. These are the FP-relevant foreign functions that can
//! be implemented as pure [`HostFn`] impls — they operate on already-evaluated
//! argument values and need no access to the AST, node payload, or scope.
//!
//! - `fn.fanout_fn(a, b)` → `[a, b]` — eager fanout into a pair/tuple.
//! - `field_access(target, field)` → `target[field]` — dynamic field access
//!   on an object (string key) or array (numeric index).
//!
//! Constructs that require lazy evaluation or payload access (`match`,
//! `handle`, `json`) are implemented as engine `Prim` ops, not `Foreign` host
//! functions, because the [`HostFn`] trait (`call(&[Value])`) provides no
//! access to the evaluation context.

use handl_language::{LangError, Value};
use handl_plugin_engine_eval::{HostFn, HostFunctions};

use std::rc::Rc;

// ---------------------------------------------------------------------------
// fn.fanout_fn(a, b) → [a, b]
// ---------------------------------------------------------------------------

/// `fn.fanout_fn(a, b)` host function — eager fanout into a pair.
///
/// Given two evaluated values, returns a JSON array `[a, b]`. This is the
/// eager interpretation of the `&&&` (fanout) operator: both sides are
/// evaluated and collected into a tuple. The higher-order interpretation
/// (composing two functions into a fanout function) requires first-class
/// closure construction, which is not available from a plain [`HostFn`].
pub struct FanoutHostFn;

impl HostFn for FanoutHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        if args.len() != 2 {
            return Err(LangError::Engine(format!(
                "fn.fanout_fn: expected 2 arguments, got {}",
                args.len()
            )));
        }
        Ok(Value::Array(args.to_vec()))
    }
}

// ---------------------------------------------------------------------------
// field_access(target, field) → target[field]
// ---------------------------------------------------------------------------

/// `field_access(target, field)` host function — dynamic field access.
///
/// If `target` is an object and `field` is a string, returns
/// `target[field]` (or `Null` if the key is absent). If `target` is an array
/// and `field` is a non-negative integer, returns `target[field]` (or an
/// error if out of bounds).
///
/// The field name arrives as the evaluated second argument. For source-parsed
/// `a.b`, the identifier `b` evaluates to the string `"b"` (via the unbound-
/// variable fallback in the tree-walk engine), so `field_access(a, "b")`
/// yields `a["b"]`.
pub struct FieldAccessHostFn;

impl HostFn for FieldAccessHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        if args.len() != 2 {
            return Err(LangError::Engine(format!(
                "field_access: expected 2 arguments (target, field), got {}",
                args.len()
            )));
        }
        let target = &args[0];
        let field = &args[1];
        match (target, field) {
            (Value::Object(obj), Value::String(key)) => {
                Ok(obj.get(key).cloned().unwrap_or(Value::Null))
            }
            (Value::Array(arr), Value::Number(n)) => {
                let i = n.as_u64().ok_or_else(|| {
                    LangError::Engine("field_access: array index must be a non-negative integer".to_string())
                })? as usize;
                arr.get(i).cloned().ok_or_else(|| {
                    LangError::Engine(format!("field_access: index {} out of bounds (len {})", i, arr.len()))
                })
            }
            _ => Err(LangError::Engine(format!(
                "field_access: target must be object/array, field must be string/number; got {} and {}",
                target, field
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// FpHostFunctions — registry of FP host functions
// ---------------------------------------------------------------------------

/// A [`HostFunctions`] registry containing the FP host functions
/// (`fn.fanout_fn`, `field_access`), with an optional delegate for further
/// delegation.
pub struct FpHostFunctions {
    fanout: FanoutHostFn,
    field_access: FieldAccessHostFn,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl Clone for FpHostFunctions {
    fn clone(&self) -> Self {
        Self {
            fanout: FanoutHostFn,
            field_access: FieldAccessHostFn,
            delegate: self.delegate.clone(),
        }
    }
}

impl FpHostFunctions {
    /// Create a new `FpHostFunctions` with an optional delegate for
    /// unknown-name lookup.
    pub fn new(delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self {
            fanout: FanoutHostFn,
            field_access: FieldAccessHostFn,
            delegate,
        }
    }
}

impl HostFunctions for FpHostFunctions {
    fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
        match name {
            "fn.fanout_fn" => Some(&self.fanout),
            "field_access" => Some(&self.field_access),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// Build an [`FpHostFunctions`] for the canonical handl language, layered on
/// top of an optional delegate.
pub fn canonical_fp_host_functions(
    delegate: Option<Rc<dyn HostFunctions>>,
) -> Rc<FpHostFunctions> {
    Rc::new(FpHostFunctions::new(delegate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn fanout_returns_pair() {
        let f = FanoutHostFn;
        let result = f.call(&[json!(1), json!(2)]).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[test]
    fn fanout_wrong_arity_errors() {
        let f = FanoutHostFn;
        let err = f.call(&[json!(1)]).unwrap_err();
        assert!(err.to_string().contains("fn.fanout_fn"));
    }

    #[test]
    fn field_access_object_string_key() {
        let f = FieldAccessHostFn;
        let result = f.call(&[json!({"a": 1, "b": 2}), json!("a")]).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn field_access_object_missing_key_returns_null() {
        let f = FieldAccessHostFn;
        let result = f.call(&[json!({"a": 1}), json!("z")]).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn field_access_array_numeric_index() {
        let f = FieldAccessHostFn;
        let result = f.call(&[json!([10, 20, 30]), json!(1)]).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn field_access_array_out_of_bounds_errors() {
        let f = FieldAccessHostFn;
        let err = f.call(&[json!([1, 2]), json!(5)]).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn field_access_wrong_types_errors() {
        let f = FieldAccessHostFn;
        let err = f.call(&[json!(42), json!("a")]).unwrap_err();
        assert!(err.to_string().contains("target must be"));
    }

    #[test]
    fn fp_host_functions_lookup_registered() {
        let fp = FpHostFunctions::new(None);
        let f = fp.lookup("fn.fanout_fn").expect("fanout should be registered");
        let result = f.call(&[json!("x"), json!("y")]).unwrap();
        assert_eq!(result, json!(["x", "y"]));

        let f = fp.lookup("field_access").expect("field_access should be registered");
        let result = f.call(&[json!({"k": 42}), json!("k")]).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn fp_host_functions_lookup_unregistered_returns_none() {
        let fp = FpHostFunctions::new(None);
        assert!(fp.lookup("missing").is_none());
    }

    #[test]
    fn fp_host_functions_delegates_unknown_names() {
        struct DummyHost;
        impl HostFn for DummyHost {
            fn call(&self, _args: &[Value]) -> Result<Value, LangError> {
                Ok(json!("delegated"))
            }
        }
        struct DummyRegistry {
            fns: HashMap<String, Box<dyn HostFn>>,
        }
        impl HostFunctions for DummyRegistry {
            fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
                self.fns.get(name).map(|f| f.as_ref())
            }
        }
        let mut dummy = DummyRegistry { fns: HashMap::new() };
        dummy.fns.insert("custom".to_string(), Box::new(DummyHost));
        let delegate: Rc<dyn HostFunctions> = Rc::new(dummy);
        let fp = FpHostFunctions::new(Some(delegate));
        let f = fp.lookup("custom").expect("should delegate");
        assert_eq!(f.call(&[]).unwrap(), json!("delegated"));
    }
}
