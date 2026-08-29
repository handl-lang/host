//! Cancellation backend for `handl-plugin-lib-concurrent`.
//!
//! The [`CancelRuntime`] trait is a **replaceable backend** for the
//! cancellation primitives exposed by [`ConcurrencyModule`]: `cancel_token`,
//! `cancel`, and `cancelable`. [`DefaultCancelRuntime`] is the bundled,
//! in-memory, `std`-based default.
//!
//! A host builder can swap the cancel backend by providing any
//! `Arc<dyn CancelRuntime + Send + Sync>`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use handl_plugin_vm_values_json::{Context, Error, Eval, FunctionValue, NonFunctionValue};
use serde_json::json;

/// A replaceable backend for the cancellation primitives:
/// `cancel_token`, `cancel`, and `cancelable`.
pub trait CancelRuntime: Send + Sync {
    /// Creates a new cancellation token.
    fn cancel_token(&self) -> NonFunctionValue;

    /// Marks `token` as cancelled and returns the updated token value.
    fn cancel(&self, token: NonFunctionValue) -> Result<NonFunctionValue, Error>;

    /// Returns `true` if `token` has been cancelled.
    fn is_cancelled(&self, token: &NonFunctionValue) -> bool;

    /// Returns the shared cancellation flag for `token`, if the token is known.
    fn cancel_flag(&self, token: &NonFunctionValue) -> Option<Arc<AtomicBool>>;

    /// Runs `thunk` unless `token` is already cancelled. If `token` is
    /// cancelled before the thunk starts, an error is returned. If the token
    /// is cancelled while the thunk is running, the result is wrapped in an
    /// object that records the cancellation.
    fn cancelable(
        &self,
        token: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error>;
}

fn token_value(id: u64, cancelled: bool) -> NonFunctionValue {
    NonFunctionValue::new(json!({
        "type": "cancel_token",
        "id": id,
        "cancelled": cancelled,
    }))
}

fn token_id(token: &NonFunctionValue) -> Result<u64, Error> {
    let obj = token
        .as_object()
        .ok_or_else(|| Error::PrimitiveError("expected a cancel token".to_string()))?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::PrimitiveError("cancel token missing 'type'".to_string()))?;
    if kind != "cancel_token" {
        return Err(Error::PrimitiveError(format!(
            "expected cancel_token, got {kind}"
        )));
    }
    obj.get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::PrimitiveError("cancel token missing numeric 'id'".to_string()))
}

fn poison<T>(_: std::sync::PoisonError<T>) -> Error {
    Error::PrimitiveError("cancel runtime lock poisoned".to_string())
}

/// An in-memory, `std`-based cancellation runtime.
#[derive(Debug)]
pub struct DefaultCancelRuntime {
    next_id: AtomicU64,
    flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl DefaultCancelRuntime {
    /// Creates a new empty cancellation runtime.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            flags: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for DefaultCancelRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelRuntime for DefaultCancelRuntime {
    fn cancel_token(&self) -> NonFunctionValue {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let flag = Arc::new(AtomicBool::new(false));
        self.flags.lock().unwrap().insert(id, flag);
        token_value(id, false)
    }

    fn cancel(&self, token: NonFunctionValue) -> Result<NonFunctionValue, Error> {
        let id = token_id(&token)?;
        let flags = self.flags.lock().map_err(poison)?;
        let flag = flags
            .get(&id)
            .ok_or_else(|| Error::PrimitiveError(format!("cancel token {id} not found")))?;
        flag.store(true, Ordering::SeqCst);
        Ok(token_value(id, true))
    }

    fn is_cancelled(&self, token: &NonFunctionValue) -> bool {
        if let Some(id) = token_id(token).ok() {
            if let Ok(flags) = self.flags.lock() {
                if let Some(flag) = flags.get(&id) {
                    return flag.load(Ordering::SeqCst);
                }
            }
        }
        false
    }

    fn cancel_flag(&self, token: &NonFunctionValue) -> Option<Arc<AtomicBool>> {
        token_id(token).ok().and_then(|id| {
            self.flags.lock().ok()?.get(&id).cloned()
        })
    }

    fn cancelable(
        &self,
        token: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let flag = self
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("expected a valid cancel token".to_string()))?;

        if flag.load(Ordering::SeqCst) {
            return Err(Error::PrimitiveError("cancelled".to_string()));
        }

        let result = ctx.eval_expr(&thunk.body, &[])?;

        if flag.load(Ordering::SeqCst) {
            return Ok(NonFunctionValue::new(json!({
                "value": result.inner().clone(),
                "cancelled": true,
            })));
        }

        Ok(result)
    }
}

/// An `Eval` wrapper that checks a shared cancellation flag before every
/// expression evaluation, aborting with `cancelled` when the flag is set.
pub struct CancelableEval {
    inner: Arc<dyn Eval<NonFunctionValue> + Send + Sync>,
    flag: Arc<AtomicBool>,
}

impl Eval<NonFunctionValue> for CancelableEval {
    fn eval_expr(
        &self,
        expr: &str,
        vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, Error> {
        if self.flag.load(Ordering::SeqCst) {
            return Err(Error::PrimitiveError("cancelled".to_string()));
        }
        self.inner.eval_expr(expr, vars)
    }
}

impl CancelableEval {
    /// Creates a new `CancelableEval` that aborts when `flag` is `true`.
    pub fn new(
        inner: Arc<dyn Eval<NonFunctionValue> + Send + Sync>,
        flag: Arc<AtomicBool>,
    ) -> Self {
        Self { inner, flag }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_vm_values_json::{CanonicalContext, Eval};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockEval {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl Eval<NonFunctionValue> for MockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            self.calls.lock().unwrap().push(expr.to_string());
            Ok(NonFunctionValue::new(json!({ "body": expr })))
        }
    }

    fn thunk(body: &str) -> FunctionValue {
        FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: body.to_string(),
            patterns: Vec::new(),
        }
    }

    fn ctx() -> CanonicalContext {
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(MockEval::default()));
        ctx
    }

    #[test]
    fn cancel_token_and_cancelable() {
        let c = DefaultCancelRuntime::new();
        let mut ctx = ctx();

        let token = c.cancel_token();
        assert_eq!(token.get("type"), Some(&json!("cancel_token")));
        assert_eq!(token.get("cancelled"), Some(&json!(false)));

        let cancelled = c.cancel(token.clone()).unwrap();
        assert_eq!(cancelled.get("cancelled"), Some(&json!(true)));

        let err = c.cancelable(cancelled, thunk("work"), &mut ctx);
        assert!(matches!(err, Err(Error::PrimitiveError(_))));

        let fresh = c.cancel_token();
        let result = c.cancelable(fresh, thunk("ok"), &mut ctx).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "ok" })));
    }

    struct PanicEval;

    impl Eval<NonFunctionValue> for PanicEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            panic!("inner eval should not be called");
        }
    }

    #[derive(Default)]
    struct SeqMockEval {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl Eval<NonFunctionValue> for SeqMockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(expr.to_string());
            let call = calls.len();
            drop(calls);

            if call == 1 {
                Ok(NonFunctionValue::new(json!("inner")))
            } else {
                panic!("inner eval should not be called after the first call");
            }
        }
    }

    #[test]
    fn cancelable_eval_aborts_when_flag_set() {
        let flag = Arc::new(AtomicBool::new(true));
        let eval = CancelableEval::new(Arc::new(PanicEval), flag);

        let err = eval.eval_expr("anything", &[]).unwrap_err();
        assert!(matches!(err, Error::PrimitiveError(ref m) if m == "cancelled"));
    }

    #[test]
    fn cancelable_eval_delegates_when_flag_clear() {
        let flag = Arc::new(AtomicBool::new(false));
        let inner = Arc::new(MockEval::default());
        let eval = CancelableEval::new(inner.clone(), flag);

        let result = eval.eval_expr("work", &[]).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "work" })));
        assert_eq!(inner.calls.lock().unwrap().as_slice(), &["work"]);
    }

    #[test]
    fn cancel_during_nested_eval() {
        let flag = Arc::new(AtomicBool::new(false));
        let inner = Arc::new(SeqMockEval::default());
        let eval = CancelableEval::new(inner.clone(), flag.clone());

        let inner_expr = eval.eval_expr("first", &[]).unwrap();
        let inner_expr = inner_expr.as_str().unwrap();
        assert_eq!(inner.calls.lock().unwrap().as_slice(), &["first"]);

        flag.store(true, Ordering::SeqCst);

        let err = eval.eval_expr(inner_expr, &[]).unwrap_err();
        assert!(matches!(err, Error::PrimitiveError(ref m) if m == "cancelled"));
        assert_eq!(inner.calls.lock().unwrap().as_slice(), &["first"]);
    }

    #[test]
    fn cancel_already_cancelled_token_is_error_or_noop() {
        let c = DefaultCancelRuntime::new();
        let token = c.cancel_token();
        c.cancel(token.clone()).unwrap();

        let again = c.cancel(token);
        // Cancelling an already-cancelled token is a no-op, not an error.
        assert!(again.is_ok());
    }
}
