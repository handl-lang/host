//! Error-path tests for `handl-lib-concurrent` backends and module resolution.

use crate::ConcurrencyModule;
use crate::atomic::{AtomicRuntime, DefaultAtomicRuntime};
use crate::scheduler::{DefaultScheduler, Scheduler};
use crate::sync::{DefaultSyncRuntime, SyncRuntime};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{CanonicalContext, Error, Eval, FunctionValue, NonFunctionValue};
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
            captured: Vec::new(),
    }
}

fn ctx() -> CanonicalContext {
    let mut ctx = CanonicalContext::new();
    ctx.set_eval(Box::new(MockEval::default()));
    ctx
}

#[test]
fn send_to_closed_channel_returns_error() {
    let s = DefaultSyncRuntime::new();
    let ch = s.channel(None);
    s.close(ch.clone()).unwrap();
    assert!(s.send(ch, NonFunctionValue::new(json!(42))).is_err());
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
    let sem = s.semaphore(NonFunctionValue::new(json!(3)));
    assert!(s.acquire(sem, NonFunctionValue::new(json!(5))).is_err());
}

#[test]
fn lock_with_bad_value_returns_error() {
    let s = DefaultSyncRuntime::new();
    let bad = NonFunctionValue::new(json!(42));
    assert!(s.lock(bad, thunk("work"), &mut ctx()).is_err());
}

#[test]
fn atomic_read_on_missing_id_returns_error() {
    let a = DefaultAtomicRuntime::new();
    let missing = NonFunctionValue::new(json!({ "type": "atomic", "id": 999 }));
    assert!(a.atomic_read(missing).is_err());
}

#[test]
fn timeout_on_missing_task_returns_error() {
    let s = DefaultScheduler::new();
    let missing = NonFunctionValue::new(json!({ "type": "task", "id": 999 }));
    let mut ctx = ctx();
    assert!(s.timeout(missing, 10, &mut ctx).is_err());
}

#[test]
fn unknown_function_returns_none() {
    let module = ConcurrencyModule::new();
    assert!(module.function("unknown").is_none());
}
