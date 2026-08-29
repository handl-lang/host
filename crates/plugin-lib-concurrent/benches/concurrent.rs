use criterion::{Criterion, criterion_group, criterion_main};
use handl_plugin_lib_concurrent::scheduler::{
    DefaultScheduler, Scheduler, ThreadPoolScheduler, WorkStealingScheduler,
};
use handl_plugin_vm_values_json::{CanonicalContext, Error, Eval, FunctionValue, NonFunctionValue};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default)]
struct MockEval;

impl Eval<NonFunctionValue> for MockEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, Error> {
        if let Ok(n) = expr.parse::<u64>() {
            return Ok(NonFunctionValue::new(json!(n)));
        }
        Ok(NonFunctionValue::new(json!({ "body": expr })))
    }
}

fn work(body: &str) -> FunctionValue {
    FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
    }
}

fn scheduler_benches(c: &mut Criterion) {
    let mut g = c.benchmark_group("scheduler");

    g.bench_function("spawn_await_empty/default", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_eval(Box::new(MockEval));
            let s = DefaultScheduler::new();
            let task = s.spawn(work("0"), &mut ctx).unwrap();
            s.await_task(task, &mut ctx).unwrap()
        })
    });

    g.bench_function("spawn_await_empty/thread_pool", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_sync_eval(Arc::new(MockEval));
            let s = ThreadPoolScheduler::new(4);
            let task = s.spawn(work("0"), &mut ctx).unwrap();
            let result = s.await_task(task, &mut ctx).unwrap();
            let _ = s.shutdown();
            result
        })
    });

    g.bench_function("spawn_await_empty/work_stealing", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_sync_eval(Arc::new(MockEval));
            let s = WorkStealingScheduler::new(4);
            let task = s.spawn(work("0"), &mut ctx).unwrap();
            let result = s.await_task(task, &mut ctx).unwrap();
            let _ = s.shutdown();
            result
        })
    });

    g.bench_function("spawn_await_many/default", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_eval(Box::new(MockEval));
            let s = DefaultScheduler::new();
            let mut tasks = Vec::with_capacity(100);
            for _ in 0..100 {
                tasks.push(s.spawn(work("0"), &mut ctx).unwrap());
            }
            for task in tasks {
                let _ = s.await_task(task, &mut ctx).unwrap();
            }
        })
    });

    g.bench_function("spawn_await_many/thread_pool", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_sync_eval(Arc::new(MockEval));
            let s = ThreadPoolScheduler::new(4);
            let mut tasks = Vec::with_capacity(100);
            for _ in 0..100 {
                tasks.push(s.spawn(work("0"), &mut ctx).unwrap());
            }
            for task in tasks {
                let _ = s.await_task(task, &mut ctx).unwrap();
            }
            let _ = s.shutdown();
        })
    });

    g.bench_function("spawn_await_many/work_stealing", |b| {
        b.iter(|| {
            let mut ctx = CanonicalContext::new();
            ctx.set_sync_eval(Arc::new(MockEval));
            let s = WorkStealingScheduler::new(4);
            let mut tasks = Vec::with_capacity(100);
            for _ in 0..100 {
                tasks.push(s.spawn(work("0"), &mut ctx).unwrap());
            }
            for task in tasks {
                let _ = s.await_task(task, &mut ctx).unwrap();
            }
            let _ = s.shutdown();
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(1));
    targets = scheduler_benches
}

criterion_main!(benches);
