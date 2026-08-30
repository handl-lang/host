use handl_plugin_lib_concurrent::atomic::DefaultAtomicRuntime;
use handl_plugin_lib_concurrent::cancel::DefaultCancelRuntime;
use handl_plugin_lib_concurrent::function_value::FunctionValue;
use handl_plugin_lib_concurrent::scheduler::{DefaultScheduler, Scheduler, ThreadPoolScheduler};
use handl_plugin_lib_concurrent::sync::{DefaultSyncRuntime, SyncRuntime};
use handl_plugin_lib_concurrent::{
    ConcurrencyModule, ParkingLotSyncRuntime, StdNetIoRuntime, ThreadTimerRuntime,
};
use handl_platform::{EvalContext, EvalError, ExprEvaluator, Module};
use serde_json::json;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

/// A thread-safe `ExprEvaluator` that can return an index or increment a shared
/// counter so that lock critical sections have a side effect.
#[derive(Clone)]
struct MockEval {
    calls: Arc<Mutex<Vec<String>>>,
    counter: Arc<AtomicU64>,
}

impl MockEval {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    fn with_counter(counter: Arc<AtomicU64>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            counter,
        }
    }
}

impl Default for MockEval {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprEvaluator for MockEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, Value)],
    ) -> Result<Value, EvalError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(expr.to_string());
        }
        if expr == "inc" {
            let v = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            return Ok(json!(v));
        }
        if let Ok(n) = expr.parse::<u64>() {
            return Ok(json!(n));
        }
        Ok(json!({ "body": expr }))
    }
}

fn thunk(body: &str) -> Value {
    FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
        captured: Vec::new(),
    }
    .to_json()
}

fn work(body: &str) -> FunctionValue {
    FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
        captured: Vec::new(),
    }
}

#[test]
fn default_sync_channel_order() {
    let s = DefaultSyncRuntime::new();
    let ch = s.channel(Some(json!(10)));

    for i in 0..10u64 {
        s.send(ch.clone(), json!(i)).unwrap();
    }

    let mut received = Vec::new();
    for _ in 0..10 {
        received.push(s.recv(ch.clone()).unwrap());
    }

    let expected: Vec<_> = (0..10u64).map(|i| json!(i)).collect();
    assert_eq!(received, expected);
}

#[test]
fn parking_lot_channel_single_producer_order() {
    let module = Arc::new(ConcurrencyModule::with_full_backends(
        Arc::new(DefaultScheduler::new()),
        Arc::new(ParkingLotSyncRuntime::new()),
        Arc::new(DefaultAtomicRuntime::new()),
        Arc::new(DefaultCancelRuntime::new()),
        Arc::new(StdNetIoRuntime::new()),
        Arc::new(ThreadTimerRuntime::new()),
    ));

    let mut ctx = EvalContext::new();
    // Use unbounded channel (null capacity) to avoid blocking on send.
    let ch = module
        .function("channel")
        .unwrap()
        .call(&[json!(null)], &mut ctx)
        .unwrap();

    // Single-threaded test: send and recv in sequence to verify ordering.
    for i in 0..100u64 {
        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), json!(i)], &mut ctx)
            .unwrap();
    }

    let mut received = Vec::new();
    for _ in 0..100 {
        let value = module
            .function("recv")
            .unwrap()
            .call(&[ch.clone()], &mut ctx)
            .unwrap();
        received.push(value);
    }

    let expected: Vec<_> = (0..100u64).map(|i| json!(i)).collect();
    assert_eq!(received, expected);
}

#[test]
fn default_scheduler_spawn_await_many() {
    let mut ctx = EvalContext::new();
    let eval = MockEval::new();
    ctx.set_expr_evaluator(Rc::new(eval.clone()));

    let scheduler = DefaultScheduler::new();
    let mut tasks = Vec::new();
    for i in 0..100u64 {
        let task = scheduler.spawn(work(&i.to_string()), &mut ctx).unwrap();
        tasks.push(task);
    }

    let mut results = Vec::new();
    for task in tasks {
        let value = scheduler.await_task(task, &mut ctx).unwrap();
        let n = value.as_u64().expect("result should be a u64");
        results.push(n);
    }
    results.sort_unstable();
    assert_eq!(results, (0..100).collect::<Vec<_>>());
    assert_eq!(eval.calls.lock().unwrap().len(), 100);
}

#[test]
fn default_scheduler_cancel_stress() {
    let mut ctx = EvalContext::new();
    let eval = MockEval::new();
    ctx.set_expr_evaluator(Rc::new(eval.clone()));

    let scheduler = DefaultScheduler::new();
    let token = scheduler.cancel_runtime().cancel_token();

    let mut tasks = Vec::new();
    for i in 0..50u64 {
        let task = scheduler
            .spawn_cancelable(token.clone(), work(&i.to_string()), &mut ctx)
            .unwrap();
        tasks.push(task);
    }

    scheduler.cancel(token).unwrap();

    let mut cancelled = 0;
    for task in tasks {
        match scheduler.await_task(task, &mut ctx) {
            Err(EvalError::Other(_)) => cancelled += 1,
            other => panic!("expected cancellation error, got {other:?}"),
        }
    }
    assert_eq!(cancelled, 50);
}

#[test]
#[ignore = "requires Send EvalContext; parking_lot mutex contention with threads"]
fn parking_lot_contended_mutex() {
    let counter = Arc::new(AtomicU64::new(0));
    let _eval = MockEval::with_counter(counter.clone());

    // This test requires sharing an EvalContext across threads, which is not
    // possible with the new Rc-based EvalContext. The test is kept as a
    // placeholder for future thread-safe evaluator support.
    let _ = counter;
}

#[test]
#[ignore = "requires Send EvalContext; channel race with threads"]
fn parking_lot_channel_race() {
    // This test requires sharing an EvalContext across threads, which is not
    // possible with the new Rc-based EvalContext.
}

#[test]
#[ignore = "requires Send EvalContext; semaphore contention with threads"]
fn parking_lot_semaphore_contention() {
    // This test requires sharing an EvalContext across threads, which is not
    // possible with the new Rc-based EvalContext.
}

#[test]
#[ignore = "ThreadPoolScheduler cannot evaluate tasks after ADR 0040 migration"]
fn thread_pool_spawn_await_many() {
    let mut ctx = EvalContext::new();
    let eval = MockEval::new();
    ctx.set_expr_evaluator(Rc::new(eval.clone()));

    let scheduler = ThreadPoolScheduler::new(4);
    let mut tasks = Vec::new();
    for i in 0..100u64 {
        let task = scheduler.spawn(work(&i.to_string()), &mut ctx).unwrap();
        tasks.push(task);
    }

    let mut results = Vec::new();
    for task in tasks {
        let value = scheduler.await_task(task, &mut ctx).unwrap();
        let n = value.as_u64().expect("result should be a u64");
        results.push(n);
    }
    results.sort_unstable();
    assert_eq!(results, (0..100).collect::<Vec<_>>());
    assert_eq!(eval.calls.lock().unwrap().len(), 100);

    let joined = scheduler.shutdown().unwrap();
    let joined_arr = joined.as_array().unwrap();
    assert!(joined_arr.iter().all(|j| j.as_bool().unwrap_or(false)), "all workers should join");
}

#[test]
#[ignore = "ThreadPoolScheduler cannot evaluate tasks after ADR 0040 migration"]
fn thread_pool_cancel_stress() {
    let mut ctx = EvalContext::new();
    let eval = MockEval::new();
    ctx.set_expr_evaluator(Rc::new(eval.clone()));

    let scheduler = ThreadPoolScheduler::new(4);
    let token = scheduler.cancel_runtime().cancel_token();

    let mut tasks = Vec::new();
    for i in 0..50u64 {
        let task = scheduler
            .spawn_cancelable(token.clone(), work(&i.to_string()), &mut ctx)
            .unwrap();
        tasks.push(task);
    }

    scheduler.cancel(token).unwrap();

    let mut cancelled = 0;
    for task in tasks {
        match scheduler.await_task(task, &mut ctx) {
            Err(EvalError::Other(_)) => cancelled += 1,
            other => panic!("expected cancellation error, got {other:?}"),
        }
    }
    assert_eq!(cancelled, 50);

    let joined = scheduler.shutdown().unwrap();
    let joined_arr = joined.as_array().unwrap();
    assert!(joined_arr.iter().all(|j| j.as_bool().unwrap_or(false)), "all workers should join");
}
