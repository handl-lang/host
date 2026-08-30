//! # handl-lib-concurrent
//!
//! Concurrency primitives for the handl runtime, exposed as host functions
//! and algebraic-effect operations.
//!
//! The first lego piece is the scheduler. `spawn`, `await`, and `yield`
//! are implemented as [`Function`]s backed by a swappable [`Scheduler`]
//! backend (see [`scheduler`](crate::scheduler)). The default scheduler is
//! [`DefaultScheduler`], an in-memory, single-threaded cooperative scheduler.
//!
//! The second lego piece is the sync backend. The synchronization
//! primitives (`channel`, `send`, `recv`, `mutex`, `lock`, `unlock`,
//! `rwlock`, `read_lock`, `write_lock`, `semaphore`, `acquire`,
//! `release`) are [`Function`]s backed by a swappable [`SyncRuntime`]
//! backend (see [`sync`](crate::sync)). [`DefaultSyncRuntime`] is the
//! bundled, in-memory, `std`-based implementation.
//!
//! The third lego piece is the atomic and cancellation backend. `atomic`,
//! `atomic_read`, `atomic_write`, `atomic_cas`, `cancel_token`, `cancel`,
//! and `cancelable` are now [`Function`]s backed by swappable
//! [`AtomicRuntime`] and [`CancelRuntime`] backends
//! (see [`atomic`](crate::atomic) and [`cancel`](crate::cancel)).
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleRuntime;
//! use handl_plugin_lib_concurrency::ConcurrencyModule;
//!
//! let host = ModuleRuntime::new().with(ConcurrencyModule::new());
//! ```
//!
//! ## Canonical syntax
//!
//! ```ignore
//! t = concurrent.spawn(work)
//! x = concurrent.await(t)
//!
//! [a, b] = [concurrent.await(t1), concurrent.await(t2)]
//!
//! first = concurrent.recv(ch1) ||| concurrent.recv(ch2)
//!
//! ch = concurrent.channel(10)
//! concurrent.send(ch, 42)
//! v = concurrent.recv(ch)
//!
//! concurrent.lock(m, () -> critical())
//!
//! tok = concurrent.cancel_token()
//! r = concurrent.cancelable(tok, work)
//! concurrent.cancel(tok)
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use serde_json::Value;

use crate::atomic::{AtomicRuntime, DefaultAtomicRuntime};
use crate::cancel::{CancelRuntime, DefaultCancelRuntime};
use crate::function_value::FunctionValue;
use crate::scheduler::{DefaultScheduler, Scheduler};
use crate::sync::{DefaultSyncRuntime, SyncRuntime};

pub mod atomic;
pub mod cancel;
pub mod function_value;
pub mod io;
pub mod scheduler;
pub mod sync;
pub mod sync_parking_lot;
pub mod timer;

pub use cancel::CancelableEval;
pub use io::{IoRuntime, MioIoRuntime, StdNetIoRuntime};
pub use scheduler::ThreadPoolScheduler;
pub use scheduler::WorkStealingScheduler;
pub use sync_parking_lot::ParkingLotSyncRuntime;
pub use timer::{ThreadTimerRuntime, TimerRuntime};

fn expect_args(args: &[Value], expected: usize) -> Result<(), EvalError> {
    if args.len() != expected {
        Err(EvalError::Other(format!("expected {} arguments, got {}", expected, args.len())))
    } else {
        Ok(())
    }
}

/// Runtime function for `concurrent.spawn`.
pub struct SpawnFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function for SpawnFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::Other(format!("expected 1 arguments, got {}", args.len())));
        }
        let work = FunctionValue::from_json(&args[0])
            .ok_or_else(|| EvalError::Other("spawn expected a function value".to_string()))?;
        self.scheduler.spawn(work, ctx)
    }
}

/// Runtime function for `concurrent.await`.
pub struct AwaitFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function for AwaitFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::Other(format!("expected 1 arguments, got {}", args.len())));
        }
        self.scheduler.await_task(args[0].clone(), ctx)
    }
}

/// Runtime function for `concurrent.yield`.
pub struct YieldFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function for YieldFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::Other(format!("expected 0 arguments, got {}", args.len())));
        }
        self.scheduler.yield_now(ctx)
    }
}

/// Runtime function for `concurrent.sleep`.
pub struct SleepFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function for SleepFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::Other(format!("expected 1 arguments, got {}", args.len())));
        }
        let ms = if args[0].is_null() {
            0
        } else {
            args[0]
                .as_f64()
                .and_then(|n| if n >= 0.0 { Some(n as u64) } else { None })
                .ok_or_else(|| {
                    EvalError::Other("sleep expected a non-negative number".to_string())
                })?
        };
        self.scheduler.sleep(ms, ctx)
    }
}

/// Runtime function for `concurrent.timeout`.
pub struct TimeoutFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function for TimeoutFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!("expected 2 arguments, got {}", args.len())));
        }
        let ms = if args[1].is_null() {
            0
        } else {
            args[1]
                .as_f64()
                .and_then(|n| if n >= 0.0 { Some(n as u64) } else { None })
                .ok_or_else(|| {
                    EvalError::Other("timeout expected a non-negative number".to_string())
                })?
        };
        self.scheduler.timeout(args[0].clone(), ms, ctx)
    }
}

/// Runtime function for `concurrent.may`.
///
/// Alias for `concurrent.spawn`.
pub struct MayFn {
    spawn: SpawnFn,
}

impl Function for MayFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.spawn.call(args, ctx)
    }
}

/// Runtime function for `concurrent.must`.
///
/// Alias for `concurrent.timeout`.
pub struct MustFn {
    timeout: TimeoutFn,
}

impl Function for MustFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.timeout.call(args, ctx)
    }
}

/// Runtime function for `concurrent.when`.
///
/// Alias for `concurrent.select`.
pub struct WhenFn {
    select: SyncFn,
}

impl Function for WhenFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.select.call(args, ctx)
    }
}

/// Generic host function that dispatches to the [`SyncRuntime`] backend.
pub struct SyncFn {
    sync: Arc<dyn SyncRuntime + Send + Sync>,
    name: String,
}

impl SyncFn {
    fn translate(
        &self,
        _args: &[Value],
        result: Result<Value, EvalError>,
    ) -> Result<Value, EvalError> {
        match result {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }
}

impl Function for SyncFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        match self.name.as_str() {
            "channel" => {
                expect_args(args, 1)?;
                let capacity = if args[0].is_null() {
                    None
                } else {
                    Some(args[0].clone())
                };
                Ok(self.sync.channel(capacity))
            }
            "send" => {
                expect_args(args, 2)?;
                self.translate(args, self.sync.send(args[0].clone(), args[1].clone()))
            }
            "try_send" => {
                expect_args(args, 2)?;
                self.translate(args, self.sync.try_send(args[0].clone(), args[1].clone()))
            }
            "recv" => {
                expect_args(args, 1)?;
                self.translate(args, self.sync.recv(args[0].clone()))
            }
            "select" => {
                expect_args(args, 1)?;
                let arr = args[0].as_array().ok_or_else(|| {
                    EvalError::Other("select expected an array of channels".to_string())
                })?;
                let channels: Vec<Value> = arr
                    .iter()
                    .map(|v| serde_json::json!(v.clone()))
                    .collect();
                self.translate(args, self.sync.select(&channels))
            }
            "mutex" => {
                expect_args(args, 0)?;
                Ok(self.sync.mutex())
            }
            "lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(&args[1]).ok_or_else(|| {
                    EvalError::Other("lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.lock(args[0].clone(), thunk, ctx))
            }
            "try_lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(&args[1]).ok_or_else(|| {
                    EvalError::Other("try_lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.try_lock(args[0].clone(), thunk, ctx))
            }
            "unlock" => {
                expect_args(args, 1)?;
                self.translate(args, self.sync.unlock(args[0].clone()))
            }
            "rwlock" => {
                expect_args(args, 0)?;
                Ok(self.sync.rwlock())
            }
            "read_lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(&args[1]).ok_or_else(|| {
                    EvalError::Other("read_lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.read_lock(args[0].clone(), thunk, ctx))
            }
            "write_lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(&args[1]).ok_or_else(|| {
                    EvalError::Other("write_lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.write_lock(args[0].clone(), thunk, ctx))
            }
            "semaphore" => {
                expect_args(args, 1)?;
                Ok(self.sync.semaphore(args[0].clone()))
            }
            "acquire" => {
                expect_args(args, 2)?;
                self.translate(args, self.sync.acquire(args[0].clone(), args[1].clone()))
            }
            "try_acquire" => {
                expect_args(args, 2)?;
                self.translate(
                    args,
                    self.sync.try_acquire(args[0].clone(), args[1].clone()),
                )
            }
            "release" => {
                expect_args(args, 2)?;
                self.translate(args, self.sync.release(args[0].clone(), args[1].clone()))
            }
            _ => Err(EvalError::UnknownFunction(self.name.clone()))
        }
    }
}

/// Generic host function that dispatches to the [`AtomicRuntime`] backend.
pub struct AtomicFn {
    atomic: Arc<dyn AtomicRuntime + Send + Sync>,
    name: String,
}

impl Function for AtomicFn {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        match self.name.as_str() {
            "atomic" => {
                expect_args(args, 1)?;
                self.atomic.atomic(args[0].clone())
            }
            "atomic_read" => {
                expect_args(args, 1)?;
                self.atomic.atomic_read(args[0].clone())
            }
            "atomic_write" => {
                expect_args(args, 2)?;
                self.atomic.atomic_write(args[0].clone(), args[1].clone())
            }
            "atomic_cas" => {
                expect_args(args, 3)?;
                self.atomic
                    .atomic_cas(args[0].clone(), args[1].clone(), args[2].clone())
            }
            _ => Err(EvalError::UnknownFunction(self.name.clone()))
        }
    }
}

/// Generic host function that dispatches to the [`CancelRuntime`] backend.
pub struct CancelFn {
    cancel: Arc<dyn CancelRuntime + Send + Sync>,
    scheduler: Arc<dyn Scheduler + Send + Sync>,
    name: String,
}

impl Function for CancelFn {
    fn call(
        &self,
        args: &[Value],
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        match self.name.as_str() {
            "cancel_token" => {
                expect_args(args, 0)?;
                Ok(self.cancel.cancel_token())
            }
            "cancel" => {
                expect_args(args, 1)?;
                let token = self.cancel.cancel(args[0].clone())?;
                self.scheduler.cancel(token.clone())?;
                Ok(token)
            }
            "cancelable" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(&args[1]).ok_or_else(|| {
                    EvalError::Other("cancelable expected a function value".to_string())
                })?;
                // Obtain the shared cancel flag so the scheduler and runtime are
                // observing the same token.
                let _ = self.cancel.cancel_flag(&args[0]).ok_or_else(|| {
                    EvalError::Other("cancelable expected a valid cancel token".to_string())
                })?;
                self.scheduler.spawn_cancelable(args[0].clone(), thunk, ctx)
            }
            _ => Err(EvalError::UnknownFunction(self.name.clone()))
        }
    }
}

/// Generic host function that dispatches to the [`IoRuntime`] backend.
pub struct IoFn {
    io: Arc<dyn IoRuntime + Send + Sync>,
    name: String,
}

impl Function for IoFn {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        match self.name.as_str() {
            "tcp_connect" => {
                expect_args(args, 1)?;
                self.io.tcp_connect(args[0].clone())
            }
            "tcp_send" => {
                expect_args(args, 2)?;
                self.io.tcp_send(args[0].clone(), args[1].clone())
            }
            "tcp_recv" => {
                expect_args(args, 1)?;
                self.io.tcp_recv(args[0].clone())
            }
            "tcp_close" => {
                expect_args(args, 1)?;
                self.io.tcp_close(args[0].clone())
            }
            _ => Err(EvalError::UnknownFunction(self.name.clone()))
        }
    }
}

/// Runtime module exposing `concurrent.*` concurrency functions.
pub struct ConcurrencyModule {
    #[allow(dead_code)]
    timer: Arc<dyn TimerRuntime + Send + Sync>,
    #[allow(dead_code)]
    scheduler: Arc<dyn Scheduler + Send + Sync>,
    spawn_fn: SpawnFn,
    await_fn: AwaitFn,
    yield_fn: YieldFn,
    sleep_fn: SleepFn,
    timeout_fn: TimeoutFn,
    may_fn: MayFn,
    must_fn: MustFn,
    when_fn: WhenFn,
    #[allow(dead_code)]
    sync: Arc<dyn SyncRuntime + Send + Sync>,
    sync_fns: HashMap<String, SyncFn>,
    #[allow(dead_code)]
    atomic: Arc<dyn AtomicRuntime + Send + Sync>,
    atomic_fns: HashMap<String, AtomicFn>,
    #[allow(dead_code)]
    cancel: Arc<dyn CancelRuntime + Send + Sync>,
    cancel_fns: HashMap<String, CancelFn>,
    #[allow(dead_code)]
    io: Arc<dyn IoRuntime + Send + Sync>,
    io_fns: HashMap<String, IoFn>,
}

impl ConcurrencyModule {
    /// Creates a concurrency module with all standard functions and the default backends.
    pub fn new() -> Self {
        let cancel = Arc::new(DefaultCancelRuntime::new());
        let timer = Arc::new(ThreadTimerRuntime::new());
        let scheduler = Arc::new(DefaultScheduler::with_timer(cancel.clone(), timer.clone()));
        Self::with_full_backends(
            scheduler,
            Arc::new(DefaultSyncRuntime::new()),
            Arc::new(DefaultAtomicRuntime::new()),
            cancel,
            Arc::new(StdNetIoRuntime::new()),
            timer,
        )
    }

    /// Creates a concurrency module with a custom [`Scheduler`] backend.
    pub fn with_scheduler(scheduler: Arc<dyn Scheduler + Send + Sync>) -> Self {
        Self::with_full_backends(
            scheduler,
            Arc::new(DefaultSyncRuntime::new()),
            Arc::new(DefaultAtomicRuntime::new()),
            Arc::new(DefaultCancelRuntime::new()),
            Arc::new(StdNetIoRuntime::new()),
            Arc::new(ThreadTimerRuntime::new()),
        )
    }

    /// Creates a concurrency module with custom scheduler and sync backends,
    /// using the default atomic and cancellation backends.
    pub fn with_backends(
        scheduler: Arc<dyn Scheduler + Send + Sync>,
        sync: Arc<dyn SyncRuntime + Send + Sync>,
    ) -> Self {
        Self::with_full_backends(
            scheduler,
            sync,
            Arc::new(DefaultAtomicRuntime::new()),
            Arc::new(DefaultCancelRuntime::new()),
            Arc::new(StdNetIoRuntime::new()),
            Arc::new(ThreadTimerRuntime::new()),
        )
    }

    /// Creates a concurrency module with fully custom backends.
    pub fn with_full_backends(
        scheduler: Arc<dyn Scheduler + Send + Sync>,
        sync: Arc<dyn SyncRuntime + Send + Sync>,
        atomic: Arc<dyn AtomicRuntime + Send + Sync>,
        cancel: Arc<dyn CancelRuntime + Send + Sync>,
        io: Arc<dyn IoRuntime + Send + Sync>,
        timer: Arc<dyn TimerRuntime + Send + Sync>,
    ) -> Self {
        let mut sync_fns = HashMap::new();
        for name in SYNC_FUNCTIONS {
            sync_fns.insert(
                name.to_string(),
                SyncFn {
                    sync: sync.clone(),
                    name: name.to_string(),
                },
            );
        }

        let mut atomic_fns = HashMap::new();
        for name in ATOMIC_FUNCTIONS {
            atomic_fns.insert(
                name.to_string(),
                AtomicFn {
                    atomic: atomic.clone(),
                    name: name.to_string(),
                },
            );
        }

        let mut cancel_fns = HashMap::new();
        for name in CANCEL_FUNCTIONS {
            cancel_fns.insert(
                name.to_string(),
                CancelFn {
                    cancel: cancel.clone(),
                    scheduler: scheduler.clone(),
                    name: name.to_string(),
                },
            );
        }

        let mut io_fns = HashMap::new();
        for name in IO_FUNCTIONS {
            io_fns.insert(
                name.to_string(),
                IoFn {
                    io: io.clone(),
                    name: name.to_string(),
                },
            );
        }

        Self {
            timer,
            scheduler: scheduler.clone(),
            spawn_fn: SpawnFn {
                scheduler: scheduler.clone(),
            },
            await_fn: AwaitFn {
                scheduler: scheduler.clone(),
            },
            yield_fn: YieldFn {
                scheduler: scheduler.clone(),
            },
            sleep_fn: SleepFn {
                scheduler: scheduler.clone(),
            },
            may_fn: MayFn {
                spawn: SpawnFn {
                    scheduler: scheduler.clone(),
                },
            },
            must_fn: MustFn {
                timeout: TimeoutFn {
                    scheduler: scheduler.clone(),
                },
            },
            timeout_fn: TimeoutFn { scheduler },
            when_fn: WhenFn {
                select: SyncFn {
                    sync: sync.clone(),
                    name: "select".to_string(),
                },
            },
            sync,
            sync_fns,
            atomic,
            atomic_fns,
            cancel,
            cancel_fns,
            io,
            io_fns,
        }
    }

    /// Returns the names of all signal-only operations exposed by this module.
    ///
    /// All concurrency primitives are now host functions, so this iterator is
    /// empty.
    pub fn operation_names(&self) -> impl Iterator<Item = &'static str> {
        std::iter::empty::<&'static str>()
    }
}

impl Default for ConcurrencyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ConcurrencyModule {
    fn name(&self) -> &str {
        "concurrent"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "spawn" => Some(&self.spawn_fn),
            "await" => Some(&self.await_fn),
            "yield" => Some(&self.yield_fn),
            "sleep" => Some(&self.sleep_fn),
            "timeout" => Some(&self.timeout_fn),
            "may" => Some(&self.may_fn),
            "must" => Some(&self.must_fn),
            "when" => Some(&self.when_fn),
            _ => self
                .sync_fns
                .get(name)
                .map(|f| f as &dyn Function)
                .or_else(|| {
                    self.atomic_fns
                        .get(name)
                        .map(|f| f as &dyn Function)
                })
                .or_else(|| {
                    self.cancel_fns
                        .get(name)
                        .map(|f| f as &dyn Function)
                })
                .or_else(|| {
                    self.io_fns
                        .get(name)
                        .map(|f| f as &dyn Function)
                }),
        }
    }
}

const SYNC_FUNCTIONS: &[&str] = &[
    "channel",
    "send",
    "try_send",
    "recv",
    "select",
    "mutex",
    "lock",
    "try_lock",
    "unlock",
    "rwlock",
    "read_lock",
    "write_lock",
    "semaphore",
    "acquire",
    "try_acquire",
    "release",
];

const ATOMIC_FUNCTIONS: &[&str] = &["atomic", "atomic_read", "atomic_write", "atomic_cas"];

const CANCEL_FUNCTIONS: &[&str] = &["cancel_token", "cancel", "cancelable"];

const IO_FUNCTIONS: &[&str] = &["tcp_connect", "tcp_send", "tcp_recv", "tcp_close"];

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::EvalContext;
    use handl_platform::ExprEvaluator;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::rc::Rc;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct MockEval {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl ExprEvaluator for MockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            self.calls.lock().unwrap().push(expr.to_string());
            Ok(serde_json::json!(json!({ "body": expr })))
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

    fn ctx() -> EvalContext {
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(MockEval::default()));
        ctx
    }

    #[test]
    fn module_name_is_concurrent() {
        assert_eq!(ConcurrencyModule::new().name(), "concurrent");
    }

    #[test]
    fn spawn_await_yield_are_functions() {
        let module = ConcurrencyModule::new();
        assert!(module.function("spawn").is_some());
        assert!(module.function("await").is_some());
        assert!(module.function("yield").is_some());
    }

    #[test]
    fn sync_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        for name in SYNC_FUNCTIONS {
            assert!(
                module.function(name).is_some(),
                "{} should be a function",
                name
            );
        }
    }

    #[test]
    fn atomic_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        for name in ATOMIC_FUNCTIONS {
            assert!(
                module.function(name).is_some(),
                "{} should be a function",
                name
            );
        }
    }

    #[test]
    fn cancel_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        for name in CANCEL_FUNCTIONS {
            assert!(
                module.function(name).is_some(),
                "{} should be a function",
                name
            );
        }
    }

    #[test]
    fn io_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        for name in IO_FUNCTIONS {
            assert!(
                module.function(name).is_some(),
                "{} should be a function",
                name
            );
        }
    }

    #[test]
    fn syntax_sugar_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        assert!(module.function("may").is_some());
        assert!(module.function("must").is_some());
        assert!(module.function("when").is_some());

        let module = ConcurrencyModule::new();
        let mut fast_ctx = EvalContext::new();
        fast_ctx.set_expr_evaluator(Rc::new(FastEval));

        let task = module
            .function("may")
            .unwrap()
            .call(&[thunk("fast")], &mut fast_ctx)
            .unwrap();
        assert_eq!(task.get("type"), Some(&json!("task")));

        let result = module
            .function("must")
            .unwrap()
            .call(&[task, serde_json::json!(json!(1000))], &mut fast_ctx)
            .unwrap();
        assert_eq!(result, serde_json::json!(json!(42)));

        let mut sync_ctx = ctx();
        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut sync_ctx)
            .unwrap();
        module
            .function("send")
            .unwrap()
            .call(
                &[ch.clone(), serde_json::json!(json!(42))],
                &mut sync_ctx,
            )
            .unwrap();
        let arg = serde_json::json!(json!([ch.clone()]));
        let got = module
            .function("when")
            .unwrap()
            .call(&[arg], &mut sync_ctx)
            .unwrap();
        assert_eq!(got, serde_json::json!(json!([0, 42])));
    }

    #[test]
    fn no_operations_are_exposed() {
        let module = ConcurrencyModule::new();
        assert_eq!(module.operation_names().count(), 0);
    }

    #[test]
    fn channel_send_recv() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();

        let value = serde_json::json!(json!(42));
        let _ = module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), value], &mut ctx)
            .unwrap();

        let got = module
            .function("recv")
            .unwrap()
            .call(&[ch.clone()], &mut ctx)
            .unwrap();
        assert_eq!(got, serde_json::json!(json!(42)));

        let arg = ch.clone();
        let empty = module
            .function("recv")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            empty,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn bounded_channel_send_full_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(2))], &mut ctx)
            .unwrap();

        let one = serde_json::json!(json!(1));
        let two = serde_json::json!(json!(2));
        let three = serde_json::json!(json!(3));

        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), one], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), two.clone()], &mut ctx)
                .is_ok()
        );

        let err = module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), three.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn channel_recv_empty_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();

        let arg = ch.clone();
        let err = module
            .function("recv")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn select_returns_first_available_channel() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch1 = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();
        let ch2 = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();

        module
            .function("send")
            .unwrap()
            .call(&[ch2.clone(), serde_json::json!(json!(42))], &mut ctx)
            .unwrap();

        let arg = serde_json::json!(json!([ch1.clone(), ch2.clone()]));
        let got = module
            .function("select")
            .unwrap()
            .call(&[arg], &mut ctx)
            .unwrap();
        assert_eq!(got, serde_json::json!(json!([1, 42])));
    }

    #[test]
    fn select_would_block_when_all_empty() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch1 = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();
        let ch2 = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(null))], &mut ctx)
            .unwrap();

        let arg = serde_json::json!(json!([ch1.clone(), ch2.clone()]));
        let err = module
            .function("select")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn bounded_channel_round_trip() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(2))], &mut ctx)
            .unwrap();

        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), serde_json::json!(json!(1))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), serde_json::json!(json!(2))], &mut ctx)
                .is_ok()
        );

        let three = serde_json::json!(json!(3));
        assert_eq!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), three.clone()], &mut ctx),
            Err(EvalError::Other("would block".to_string()))
        );

        let got = module
            .function("recv")
            .unwrap()
            .call(&[ch.clone()], &mut ctx)
            .unwrap();
        assert_eq!(got, serde_json::json!(json!(1)));

        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), serde_json::json!(json!(3))], &mut ctx)
                .is_ok()
        );

        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[ch.clone()], &mut ctx)
                .unwrap(),
            serde_json::json!(json!(2))
        );
        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[ch.clone()], &mut ctx)
                .unwrap(),
            serde_json::json!(json!(3))
        );

        let arg = ch.clone();
        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[arg.clone()], &mut ctx),
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn mutex_lock_unlock() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let m = module
            .function("mutex")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();

        let result = module
            .function("lock")
            .unwrap()
            .call(&[m.clone(), thunk("locked!")], &mut ctx)
            .unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "locked!" })));

        let err = module.function("unlock").unwrap().call(&[m], &mut ctx);
        assert!(
            matches!(err, Err(EvalError::Other(_))),
            "unlock on unlocked mutex should error"
        );
    }

    #[test]
    fn semaphore_acquire_release() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let sem = module
            .function("semaphore")
            .unwrap()
            .call(&[serde_json::json!(json!(3))], &mut ctx)
            .unwrap();

        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem.clone(), serde_json::json!(json!(2))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem.clone(), serde_json::json!(json!(2))], &mut ctx)
                .is_err()
        );
        assert!(
            module
                .function("release")
                .unwrap()
                .call(&[sem.clone(), serde_json::json!(json!(2))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem, serde_json::json!(json!(2))], &mut ctx)
                .is_ok()
        );
    }

    #[test]
    fn rwlock_read_write() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let rw = module
            .function("rwlock")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();

        assert!(
            module
                .function("read_lock")
                .unwrap()
                .call(&[rw.clone(), thunk("read")], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("write_lock")
                .unwrap()
                .call(&[rw, thunk("write")], &mut ctx)
                .is_ok()
        );
    }

    #[test]
    fn atomic_read_write_cas() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let r = module
            .function("atomic")
            .unwrap()
            .call(&[serde_json::json!(json!(0))], &mut ctx)
            .unwrap();
        assert_eq!(r.get("type"), Some(&json!("atomic")));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, serde_json::json!(json!(0)));

        let old = module
            .function("atomic_write")
            .unwrap()
            .call(&[r.clone(), serde_json::json!(json!(42))], &mut ctx)
            .unwrap();
        assert_eq!(old, serde_json::json!(json!(0)));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, serde_json::json!(json!(42)));

        let swapped = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r.clone(),
                    serde_json::json!(json!(42)),
                    serde_json::json!(json!(7)),
                ],
                &mut ctx,
            )
            .unwrap();
        assert_eq!(swapped, serde_json::json!(json!(true)));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, serde_json::json!(json!(7)));

        let failed = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r,
                    serde_json::json!(json!(42)),
                    serde_json::json!(json!(9)),
                ],
                &mut ctx,
            )
            .unwrap();
        assert_eq!(failed, serde_json::json!(json!(false)));
    }

    #[test]
    fn cancel_token_and_cancelable() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        assert_eq!(token.get("type"), Some(&json!("cancel_token")));
        assert_eq!(token.get("cancelled"), Some(&json!(false)));

        let cancelled = module
            .function("cancel")
            .unwrap()
            .call(&[token.clone()], &mut ctx)
            .unwrap();
        assert_eq!(cancelled.get("cancelled"), Some(&json!(true)));

        let err = module
            .function("cancelable")
            .unwrap()
            .call(&[cancelled, thunk("work")], &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));

        let fresh = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let task = module
            .function("cancelable")
            .unwrap()
            .call(&[fresh, thunk("ok")], &mut ctx)
            .unwrap();
        let result = module
            .function("await")
            .unwrap()
            .call(&[task], &mut ctx)
            .unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "ok" })));
    }

    #[test]
    fn cancel_pending_task() {
        let module = ConcurrencyModule::new();
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let task = module
            .function("cancelable")
            .unwrap()
            .call(&[token.clone(), thunk("hello()")], &mut ctx)
            .unwrap();

        let _ = module
            .function("cancel")
            .unwrap()
            .call(&[token], &mut ctx)
            .unwrap();

        let err = module.function("await").unwrap().call(&[task], &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn cancelable_does_not_run_when_already_cancelled() {
        let module = ConcurrencyModule::new();
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let _ = module
            .function("cancel")
            .unwrap()
            .call(&[token.clone()], &mut ctx)
            .unwrap();

        let err = module
            .function("cancelable")
            .unwrap()
            .call(&[token, thunk("work")], &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn cancelable_runs_then_result_returned() {
        let module = ConcurrencyModule::new();
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let task = module
            .function("cancelable")
            .unwrap()
            .call(&[token, thunk("ok")], &mut ctx)
            .unwrap();
        let result = module
            .function("await")
            .unwrap()
            .call(&[task], &mut ctx)
            .unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "ok" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["ok"]);
    }

    #[test]
    fn try_sync_functions_are_resolved() {
        let module = ConcurrencyModule::new();
        for name in ["try_send", "try_acquire", "try_lock"] {
            assert!(
                module.function(name).is_some(),
                "{name} should be a function"
            );
        }
    }

    #[test]
    fn try_send_full_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[serde_json::json!(json!(2))], &mut ctx)
            .unwrap();

        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), serde_json::json!(json!(1))], &mut ctx)
            .unwrap();
        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), serde_json::json!(json!(2))], &mut ctx)
            .unwrap();

        let three = serde_json::json!(json!(3));
        let err = module
            .function("try_send")
            .unwrap()
            .call(&[ch.clone(), three.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[test]
    fn try_acquire_insufficient_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let sem = module
            .function("semaphore")
            .unwrap()
            .call(&[serde_json::json!(json!(2))], &mut ctx)
            .unwrap();

        let three = serde_json::json!(json!(3));
        let err = module
            .function("try_acquire")
            .unwrap()
            .call(&[sem.clone(), three.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(EvalError::Other("would block".to_string()))
        );
    }

    #[derive(Clone)]
    struct FastEval;

    impl ExprEvaluator for FastEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            if expr == "fast" {
                Ok(serde_json::json!(json!(42)))
            } else {
                Ok(serde_json::json!(json!({ "body": expr })))
            }
        }
    }

    #[derive(Clone)]
    struct SlowEval {
        flag: Arc<AtomicBool>,
    }

    impl ExprEvaluator for SlowEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            for _ in 0..200 {
                if self.flag.load(Ordering::SeqCst) {
                    return Err(EvalError::Other("cancelled".to_string()));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Ok(serde_json::json!(json!("done")))
        }
    }

    #[test]
    fn sleep_is_a_function() {
        let module = ConcurrencyModule::new();
        assert!(module.function("sleep").is_some());
    }

    #[test]
    fn timeout_returns_result_before_deadline() {
        let module = ConcurrencyModule::new();
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(FastEval));

        let task = module
            .function("spawn")
            .unwrap()
            .call(&[thunk("fast")], &mut ctx)
            .unwrap();
        let result = module
            .function("timeout")
            .unwrap()
            .call(&[task, serde_json::json!(json!(100))], &mut ctx)
            .unwrap();

        assert_eq!(result, serde_json::json!(json!(42)));
    }

    #[test]
    fn timeout_cancels_slow_task() {
        let cancel = Arc::new(DefaultCancelRuntime::new());
        let scheduler = Arc::new(DefaultScheduler::with_cancel_runtime(cancel.clone()));
        let module = ConcurrencyModule::with_full_backends(
            scheduler,
            Arc::new(DefaultSyncRuntime::new()),
            Arc::new(DefaultAtomicRuntime::new()),
            cancel.clone(),
            Arc::new(StdNetIoRuntime::new()),
            Arc::new(ThreadTimerRuntime::new()),
        );
        let mut ctx = EvalContext::new();

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let flag = cancel.cancel_flag(&token).unwrap();
        ctx.set_expr_evaluator(Rc::new(SlowEval { flag }));

        let task = module
            .function("cancelable")
            .unwrap()
            .call(&[token, thunk("slow")], &mut ctx)
            .unwrap();
        let err = module
            .function("timeout")
            .unwrap()
            .call(&[task, serde_json::json!(json!(50))], &mut ctx);

        assert!(matches!(err, Err(EvalError::Other(_))));
    }
}

#[cfg(test)]
mod error_tests;

pub mod plugin;
pub use plugin::LibraryPlugin;
