//! Error-path tests for `handl-lib-concurrent` backends and module resolution.

use crate::ConcurrencyModule;
use crate::atomic::{AtomicRuntime, DefaultAtomicRuntime};
use crate::function_value::FunctionValue;
use crate::scheduler::{DefaultScheduler, Scheduler};
use crate::sync::{DefaultSyncRuntime, SyncRuntime};
use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, ExprEvaluator};
use serde_json::Value;
use serde_json::json;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MockEval {
    calls: Arc<Mutex<Vec<String>>>,
}

impl ExprEvaluator for MockEval {
    fn eval_expr(&self, expr: &str, _vars: &[(String, Value)]) -> Result<Value, EvalError> {
        self.calls.lock().unwrap().push(expr.to_string());
        Ok(serde_json::json!(json!({ "body": expr })))
    }
}

fn thunk(body: &str) -> FunctionValue {
    FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
        captured: Vec::new(),
    }
}

fn ctx() -> EvalContext {
    let mut ctx = EvalContext::new();
    ctx.set_expr_evaluator(Rc::new(MockEval::default()));
    ctx
}

#[test]
fn send_to_closed_channel_returns_error() {
    let s = DefaultSyncRuntime::new();
    let ch = s.channel(None);
    s.close(ch.clone()).unwrap();
    assert!(s.send(ch, serde_json::json!(json!(42))).is_err());
}

#[test]
fn recv_from_closed_channel_returns_error() {
    let s = DefaultSyncRuntime::new();
    let ch = s.channel(None);
    s.close(ch.clone()).unwrap();
    assert!(s.recv(ch).is_err());
}

#[test]
fn acquire_more_than_semaphore_max_returns_error() {
    let s = DefaultSyncRuntime::new();
    let sem = s.semaphore(serde_json::json!(json!(3)));
    assert!(s.acquire(sem, serde_json::json!(json!(5))).is_err());
}

#[test]
fn lock_with_bad_value_returns_error() {
    let s = DefaultSyncRuntime::new();
    let bad = serde_json::json!(json!(42));
    assert!(s.lock(bad, thunk("work"), &mut ctx()).is_err());
}

#[test]
fn atomic_read_on_missing_id_returns_error() {
    let a = DefaultAtomicRuntime::new();
    let missing = serde_json::json!(json!({ "type": "atomic", "id": 999 }));
    assert!(a.atomic_read(missing).is_err());
}

#[test]
fn timeout_on_missing_task_returns_error() {
    let s = DefaultScheduler::new();
    let missing = serde_json::json!(json!({ "type": "task", "id": 999 }));
    let mut ctx = ctx();
    assert!(s.timeout(missing, 10, &mut ctx).is_err());
}

#[test]
fn unknown_function_returns_none() {
    let module = ConcurrencyModule::new();
    assert!(module.function("unknown").is_none());
}
