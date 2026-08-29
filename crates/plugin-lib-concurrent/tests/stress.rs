use handl_plugin_lib_concurrent::atomic::DefaultAtomicRuntime;
use handl_plugin_lib_concurrent::cancel::DefaultCancelRuntime;
use handl_plugin_lib_concurrent::scheduler::{DefaultScheduler, Scheduler, ThreadPoolScheduler};
use handl_plugin_lib_concurrent::sync::{DefaultSyncRuntime, SyncRuntime};
use handl_plugin_lib_concurrent::{
    ConcurrencyModule, ParkingLotSyncRuntime, StdNetIoRuntime, ThreadTimerRuntime,
};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{CanonicalContext, Error, Eval, FunctionValue, NonFunctionValue};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A thread-safe `Eval` that can return an index or increment a shared
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

impl Eval<NonFunctionValue> for MockEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, Error> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(expr.to_string());
        }
        if expr == "inc" {
            let v = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            return Ok(NonFunctionValue::new(json!(v)));
        }
        if let Ok(n) = expr.parse::<u64>() {
            return Ok(NonFunctionValue::new(json!(n)));
        }
        Ok(NonFunctionValue::new(json!({ "body": expr })))
    }
}

fn thunk(body: &str) -> NonFunctionValue {
    NonFunctionValue::new(FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
            captured: Vec::new(),
    })
}

#[test]
fn thread_pool_spawn_await_many() {
    let mut ctx = CanonicalContext::new();
    let eval = Arc::new(MockEval::new());
    ctx.set_sync_eval(eval.clone());

    let scheduler = ThreadPoolScheduler::new(4);
    let mut tasks = Vec::new();
    for i in 0..100u64 {
        let work = FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: i.to_string(),
            patterns: Vec::new(),
            captured: Vec::new(),
        };
        let task = scheduler.spawn(work, &mut ctx).unwrap();
        tasks.push(task);
    }

    let mut results = Vec::new();
    for task in tasks {
        let value = scheduler.await_task(task, &mut ctx).unwrap();
        let n = value.inner().as_u64().expect("result should be a u64");
        results.push(n);
    }
    results.sort_unstable();
    assert_eq!(results, (0..100).collect::<Vec<_>>());
    assert_eq!(eval.calls.lock().unwrap().len(), 100);

    let joined = scheduler.shutdown().unwrap();
    assert!(joined.iter().all(|&j| j), "all workers should join");
}

#[test]
#[ignore]
fn thread_pool_cancel_stress() {
    let mut ctx = CanonicalContext::new();
    let eval = Arc::new(MockEval::new());
    ctx.set_sync_eval(eval.clone());

    let scheduler = ThreadPoolScheduler::new(4);
    let token = scheduler.cancel_runtime().cancel_token();

    let mut tasks = Vec::new();
    for i in 0..50u64 {
        let work = FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: i.to_string(),
            patterns: Vec::new(),
            captured: Vec::new(),
        };
        let task = scheduler
            .spawn_cancelable(token.clone(), work, &mut ctx)
            .unwrap();
        tasks.push(task);
    }

    scheduler.cancel(token).unwrap();

    let mut cancelled = 0;
    for task in tasks {
        match scheduler.await_task(task, &mut ctx) {
            Err(Error::PrimitiveError(_)) => cancelled += 1,
            other => panic!("expected cancellation error, got {other:?}"),
        }
    }
    assert_eq!(cancelled, 50);

    let joined = scheduler.shutdown().unwrap();
    assert!(joined.iter().all(|&j| j), "all workers should join");
}

#[test]
#[ignore]
fn parking_lot_contended_mutex() {
    let counter = Arc::new(AtomicU64::new(0));
    let eval = Arc::new(MockEval::with_counter(counter.clone()));

    let module = Arc::new(ConcurrencyModule::with_full_backends(
        Arc::new(DefaultScheduler::new()),
        Arc::new(ParkingLotSyncRuntime::new()),
        Arc::new(DefaultAtomicRuntime::new()),
        Arc::new(DefaultCancelRuntime::new()),
        Arc::new(StdNetIoRuntime::new()),
        Arc::new(ThreadTimerRuntime::new()),
    ));

    let mut ctx = CanonicalContext::new();
    ctx.set_sync_eval(eval.clone());
    let m = module
        .function("mutex")
        .unwrap()
        .call(&[], &mut ctx)
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let module = module.clone();
        let m = m.clone();
        let eval = eval.clone();
        handles.push(thread::spawn(move || {
            let mut ctx = CanonicalContext::new();
            ctx.set_sync_eval(eval);
            for _ in 0..1000 {
                module
                    .function("lock")
                    .unwrap()
                    .call(&[m.clone(), thunk("inc")], &mut ctx)
                    .unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter.load(Ordering::SeqCst), 8000);
}

#[test]
#[ignore]
fn parking_lot_channel_race() {
    let module = Arc::new(ConcurrencyModule::with_full_backends(
        Arc::new(DefaultScheduler::new()),
        Arc::new(ParkingLotSyncRuntime::new()),
        Arc::new(DefaultAtomicRuntime::new()),
        Arc::new(DefaultCancelRuntime::new()),
        Arc::new(StdNetIoRuntime::new()),
        Arc::new(ThreadTimerRuntime::new()),
    ));

    let mut ctx = CanonicalContext::new();
    let ch = module
        .function("channel")
        .unwrap()
        .call(&[NonFunctionValue::new(json!(10))], &mut ctx)
        .unwrap();

    let mut producer_handles = Vec::new();
    for producer_id in 0..2u64 {
        let module = module.clone();
        let ch = ch.clone();
        producer_handles.push(thread::spawn(move || {
            let mut ctx = CanonicalContext::new();
            for i in 0..100u64 {
                let value = NonFunctionValue::new(json!({"p": producer_id, "i": i}));
                module
                    .function("send")
                    .unwrap()
                    .call(&[ch.clone(), value], &mut ctx)
                    .unwrap();
            }
        }));
    }

    let module_c = module.clone();
    let ch_c = ch.clone();
    let consumer_handle = thread::spawn(move || {
        let mut ctx = CanonicalContext::new();
        let mut by_producer = vec![Vec::new(); 2];
        for _ in 0..200 {
            let value = module_c
                .function("recv")
                .unwrap()
                .call(&[ch_c.clone()], &mut ctx)
                .unwrap();
            let obj = value
                .as_object()
                .expect("received value should be an object");
            let p = obj
                .get("p")
                .and_then(|v| v.as_u64())
                .expect("producer id should be a u64") as usize;
            let i = obj
                .get("i")
                .and_then(|v| v.as_u64())
                .expect("index should be a u64") as usize;
            by_producer[p].push(i);
        }
        by_producer
    });

    for h in producer_handles {
        h.join().unwrap();
    }

    let by_producer = consumer_handle.join().unwrap();

    assert_eq!(by_producer.iter().map(|v| v.len()).sum::<usize>(), 200);
    for producer_id in 0..2 {
        assert_eq!(
            by_producer[producer_id],
            (0..100u64).map(|i| i as usize).collect::<Vec<_>>(),
            "producer {producer_id} should deliver every index 0..99 exactly once in order"
        );
    }
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

    let mut ctx = CanonicalContext::new();
    let ch = module
        .function("channel")
        .unwrap()
        .call(&[NonFunctionValue::new(json!(10))], &mut ctx)
        .unwrap();

    let module_p = module.clone();
    let ch_p = ch.clone();
    let producer_handle = thread::spawn(move || {
        let mut ctx = CanonicalContext::new();
        for i in 0..1000u64 {
            let value = NonFunctionValue::new(json!(i));
            module_p
                .function("send")
                .unwrap()
                .call(&[ch_p.clone(), value], &mut ctx)
                .unwrap();
        }
    });

    let module_c = module.clone();
    let ch_c = ch.clone();
    let consumer_handle = thread::spawn(move || {
        let mut ctx = CanonicalContext::new();
        let mut received = Vec::new();
        for _ in 0..1000 {
            let value = module_c
                .function("recv")
                .unwrap()
                .call(&[ch_c.clone()], &mut ctx)
                .unwrap();
            received.push(value);
        }
        received
    });

    producer_handle.join().unwrap();
    let received = consumer_handle.join().unwrap();

    let expected: Vec<_> = (0..1000u64)
        .map(|i| NonFunctionValue::new(json!(i)))
        .collect();
    assert_eq!(received, expected);
}

#[test]
fn default_sync_channel_order() {
    let s = DefaultSyncRuntime::new();
    let ch = s.channel(Some(NonFunctionValue::new(json!(10))));

    for i in 0..10u64 {
        s.send(ch.clone(), NonFunctionValue::new(json!(i))).unwrap();
    }

    let mut received = Vec::new();
    for _ in 0..10 {
        received.push(s.recv(ch.clone()).unwrap());
    }

    let expected: Vec<_> = (0..10u64)
        .map(|i| NonFunctionValue::new(json!(i)))
        .collect();
    assert_eq!(received, expected);
}

#[test]
#[ignore]
fn parking_lot_semaphore_contention() {
    let module = Arc::new(ConcurrencyModule::with_full_backends(
        Arc::new(DefaultScheduler::new()),
        Arc::new(ParkingLotSyncRuntime::new()),
        Arc::new(DefaultAtomicRuntime::new()),
        Arc::new(DefaultCancelRuntime::new()),
        Arc::new(StdNetIoRuntime::new()),
        Arc::new(ThreadTimerRuntime::new()),
    ));

    let mut ctx = CanonicalContext::new();
    let sem = module
        .function("semaphore")
        .unwrap()
        .call(&[NonFunctionValue::new(json!(2))], &mut ctx)
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let module = module.clone();
        let sem = sem.clone();
        handles.push(thread::spawn(move || {
            let mut ctx = CanonicalContext::new();
            let one = NonFunctionValue::new(json!(1));
            for _ in 0..100 {
                module
                    .function("acquire")
                    .unwrap()
                    .call(&[sem.clone(), one.clone()], &mut ctx)
                    .unwrap();
                thread::sleep(Duration::from_micros(10));
                module
                    .function("release")
                    .unwrap()
                    .call(&[sem.clone(), one.clone()], &mut ctx)
                    .unwrap();
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not deadlock");
    }
}
