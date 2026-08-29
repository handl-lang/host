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
//! use handl_plugin_vm_common::ModuleHost;
//! use handl_plugin_lib_concurrency::ConcurrencyModule;
//!
//! let host = ModuleHost::new().with(ConcurrencyModule::new());
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

use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Context, Error, Function, FunctionValue, NonFunctionValue};

use crate::atomic::{AtomicRuntime, DefaultAtomicRuntime};
use crate::cancel::{CancelRuntime, DefaultCancelRuntime};
use crate::scheduler::{DefaultScheduler, Scheduler};
use crate::sync::{DefaultSyncRuntime, SyncError, SyncRuntime};

pub mod atomic;
pub mod cancel;
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

fn expect_args(args: &[NonFunctionValue], expected: usize) -> Result<(), Error> {
    if args.len() != expected {
        Err(Error::ArgumentCount {
            expected,
            got: args.len(),
        })
    } else {
        Ok(())
    }
}

/// Host function for `concurrent.spawn`.
pub struct SpawnFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function<NonFunctionValue> for SpawnFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() != 1 {
            return Err(Error::ArgumentCount {
                expected: 1,
                got: args.len(),
            });
        }
        let work = FunctionValue::from_json(args[0].inner())
            .ok_or_else(|| Error::PrimitiveError("spawn expected a function value".to_string()))?;
        self.scheduler.spawn(work, ctx)
    }
}

/// Host function for `concurrent.await`.
pub struct AwaitFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function<NonFunctionValue> for AwaitFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() != 1 {
            return Err(Error::ArgumentCount {
                expected: 1,
                got: args.len(),
            });
        }
        self.scheduler.await_task(args[0].clone(), ctx)
    }
}

/// Host function for `concurrent.yield`.
pub struct YieldFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function<NonFunctionValue> for YieldFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if !args.is_empty() {
            return Err(Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }
        self.scheduler.yield_now(ctx)
    }
}

/// Host function for `concurrent.sleep`.
pub struct SleepFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function<NonFunctionValue> for SleepFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() != 1 {
            return Err(Error::ArgumentCount {
                expected: 1,
                got: args.len(),
            });
        }
        let ms = if args[0].is_null() {
            0
        } else {
            args[0]
                .as_number()
                .and_then(|n| if n >= 0.0 { Some(n as u64) } else { None })
                .ok_or_else(|| {
                    Error::PrimitiveError("sleep expected a non-negative number".to_string())
                })?
        };
        self.scheduler.sleep(ms, ctx)
    }
}

/// Host function for `concurrent.timeout`.
pub struct TimeoutFn {
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl Function<NonFunctionValue> for TimeoutFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() != 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let ms = if args[1].is_null() {
            0
        } else {
            args[1]
                .as_number()
                .and_then(|n| if n >= 0.0 { Some(n as u64) } else { None })
                .ok_or_else(|| {
                    Error::PrimitiveError("timeout expected a non-negative number".to_string())
                })?
        };
        self.scheduler.timeout(args[0].clone(), ms, ctx)
    }
}

/// Host function for `concurrent.may`.
///
/// Alias for `concurrent.spawn`.
pub struct MayFn {
    spawn: SpawnFn,
}

impl Function<NonFunctionValue> for MayFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.spawn.call(args, ctx)
    }
}

/// Host function for `concurrent.must`.
///
/// Alias for `concurrent.timeout`.
pub struct MustFn {
    timeout: TimeoutFn,
}

impl Function<NonFunctionValue> for MustFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.timeout.call(args, ctx)
    }
}

/// Host function for `concurrent.when`.
///
/// Alias for `concurrent.select`.
pub struct WhenFn {
    select: SyncFn,
}

impl Function<NonFunctionValue> for WhenFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
        args: &[NonFunctionValue],
        result: Result<NonFunctionValue, SyncError>,
    ) -> Result<NonFunctionValue, Error> {
        match result {
            Ok(v) => Ok(v),
            Err(SyncError::WouldBlock)
                if matches!(
                    self.name.as_str(),
                    "send"
                        | "recv"
                        | "acquire"
                        | "select"
                        | "try_send"
                        | "try_recv"
                        | "try_acquire"
                        | "try_lock"
                ) =>
            {
                Err(Error::Operation {
                    name: self.name.clone(),
                    args: args.to_vec(),
                })
            }
            Err(e) => Err(Error::PrimitiveError(e.to_string())),
        }
    }
}

impl Function<NonFunctionValue> for SyncFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
                    Error::PrimitiveError("select expected an array of channels".to_string())
                })?;
                let channels: Vec<NonFunctionValue> = arr
                    .iter()
                    .map(|v| NonFunctionValue::new(v.clone()))
                    .collect();
                self.translate(args, self.sync.select(&channels))
            }
            "mutex" => {
                expect_args(args, 0)?;
                Ok(self.sync.mutex())
            }
            "lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(args[1].inner()).ok_or_else(|| {
                    Error::PrimitiveError("lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.lock(args[0].clone(), thunk, ctx))
            }
            "try_lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(args[1].inner()).ok_or_else(|| {
                    Error::PrimitiveError("try_lock expected a function value".to_string())
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
                let thunk = FunctionValue::from_json(args[1].inner()).ok_or_else(|| {
                    Error::PrimitiveError("read_lock expected a function value".to_string())
                })?;
                self.translate(args, self.sync.read_lock(args[0].clone(), thunk, ctx))
            }
            "write_lock" => {
                expect_args(args, 2)?;
                let thunk = FunctionValue::from_json(args[1].inner()).ok_or_else(|| {
                    Error::PrimitiveError("write_lock expected a function value".to_string())
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
            _ => Err(Error::UnknownFunction(self.name.clone())),
        }
    }
}

/// Generic host function that dispatches to the [`AtomicRuntime`] backend.
pub struct AtomicFn {
    atomic: Arc<dyn AtomicRuntime + Send + Sync>,
    name: String,
}

impl Function<NonFunctionValue> for AtomicFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
            _ => Err(Error::UnknownFunction(self.name.clone())),
        }
    }
}

/// Generic host function that dispatches to the [`CancelRuntime`] backend.
pub struct CancelFn {
    cancel: Arc<dyn CancelRuntime + Send + Sync>,
    scheduler: Arc<dyn Scheduler + Send + Sync>,
    name: String,
}

impl Function<NonFunctionValue> for CancelFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
                let thunk = FunctionValue::from_json(args[1].inner()).ok_or_else(|| {
                    Error::PrimitiveError("cancelable expected a function value".to_string())
                })?;
                // Obtain the shared cancel flag so the scheduler and runtime are
                // observing the same token.
                let _ = self.cancel.cancel_flag(&args[0]).ok_or_else(|| {
                    Error::PrimitiveError("cancelable expected a valid cancel token".to_string())
                })?;
                self.scheduler.spawn_cancelable(args[0].clone(), thunk, ctx)
            }
            _ => Err(Error::UnknownFunction(self.name.clone())),
        }
    }
}

/// Generic host function that dispatches to the [`IoRuntime`] backend.
pub struct IoFn {
    io: Arc<dyn IoRuntime + Send + Sync>,
    name: String,
}

impl Function<NonFunctionValue> for IoFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
            _ => Err(Error::UnknownFunction(self.name.clone())),
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

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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
                .map(|f| f as &dyn Function<NonFunctionValue>)
                .or_else(|| {
                    self.atomic_fns
                        .get(name)
                        .map(|f| f as &dyn Function<NonFunctionValue>)
                })
                .or_else(|| {
                    self.cancel_fns
                        .get(name)
                        .map(|f| f as &dyn Function<NonFunctionValue>)
                })
                .or_else(|| {
                    self.io_fns
                        .get(name)
                        .map(|f| f as &dyn Function<NonFunctionValue>)
                }),
        }
    }

    fn operation(
        &self,
        _name: &str,
    ) -> Option<&dyn handl_plugin_vm_values_json::Operation<NonFunctionValue>> {
        None
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
    use handl_plugin_vm_values_json::CanonicalContext;
    use handl_plugin_vm_values_json::Eval;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

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

    fn thunk(body: &str) -> NonFunctionValue {
        NonFunctionValue::new(FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: body.to_string(),
            patterns: Vec::new(),
        })
    }

    fn ctx() -> CanonicalContext {
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(MockEval::default()));
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

        let scheduler = Arc::new(ThreadPoolScheduler::new(2));
        let module = ConcurrencyModule::with_scheduler(scheduler);
        let mut fast_ctx = CanonicalContext::new();
        fast_ctx.set_sync_eval(Arc::new(FastEval));

        let task = module
            .function("may")
            .unwrap()
            .call(&[thunk("fast")], &mut fast_ctx)
            .unwrap();
        assert_eq!(task.get("type"), Some(&json!("task")));

        let result = module
            .function("must")
            .unwrap()
            .call(&[task, NonFunctionValue::new(json!(1000))], &mut fast_ctx)
            .unwrap();
        assert_eq!(result, NonFunctionValue::new(json!(42)));

        let mut sync_ctx = ctx();
        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut sync_ctx)
            .unwrap();
        module
            .function("send")
            .unwrap()
            .call(
                &[ch.clone(), NonFunctionValue::new(json!(42))],
                &mut sync_ctx,
            )
            .unwrap();
        let arg = NonFunctionValue::new(json!([ch.inner().clone()]));
        let got = module
            .function("when")
            .unwrap()
            .call(&[arg], &mut sync_ctx)
            .unwrap();
        assert_eq!(got, NonFunctionValue::new(json!([0, 42])));
    }

    #[test]
    fn no_operations_are_exposed() {
        let module = ConcurrencyModule::new();
        assert!(module.operation("atomic").is_none());
        assert!(module.operation("cancel_token").is_none());
        assert_eq!(module.operation_names().count(), 0);
    }

    #[test]
    fn channel_send_recv() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();

        let value = NonFunctionValue::new(json!(42));
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
        assert_eq!(got, NonFunctionValue::new(json!(42)));

        let arg = ch.clone();
        let empty = module
            .function("recv")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            empty,
            Err(Error::Operation {
                name: "recv".to_string(),
                args: vec![arg],
            })
        );
    }

    #[test]
    fn bounded_channel_send_full_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(2))], &mut ctx)
            .unwrap();

        let one = NonFunctionValue::new(json!(1));
        let two = NonFunctionValue::new(json!(2));
        let three = NonFunctionValue::new(json!(3));

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
            Err(Error::Operation {
                name: "send".to_string(),
                args: vec![ch.clone(), three],
            })
        );
    }

    #[test]
    fn channel_recv_empty_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();

        let arg = ch.clone();
        let err = module
            .function("recv")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(Error::Operation {
                name: "recv".to_string(),
                args: vec![arg],
            })
        );
    }

    #[test]
    fn select_returns_first_available_channel() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch1 = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();
        let ch2 = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();

        module
            .function("send")
            .unwrap()
            .call(&[ch2.clone(), NonFunctionValue::new(json!(42))], &mut ctx)
            .unwrap();

        let arg = NonFunctionValue::new(json!([ch1.inner().clone(), ch2.inner().clone()]));
        let got = module
            .function("select")
            .unwrap()
            .call(&[arg], &mut ctx)
            .unwrap();
        assert_eq!(got, NonFunctionValue::new(json!([1, 42])));
    }

    #[test]
    fn select_would_block_when_all_empty() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch1 = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();
        let ch2 = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();

        let arg = NonFunctionValue::new(json!([ch1.inner().clone(), ch2.inner().clone()]));
        let err = module
            .function("select")
            .unwrap()
            .call(&[arg.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(Error::Operation {
                name: "select".to_string(),
                args: vec![arg],
            })
        );
    }

    #[test]
    fn bounded_channel_round_trip() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(2))], &mut ctx)
            .unwrap();

        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), NonFunctionValue::new(json!(1))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), NonFunctionValue::new(json!(2))], &mut ctx)
                .is_ok()
        );

        let three = NonFunctionValue::new(json!(3));
        assert_eq!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), three.clone()], &mut ctx),
            Err(Error::Operation {
                name: "send".to_string(),
                args: vec![ch.clone(), three],
            })
        );

        let got = module
            .function("recv")
            .unwrap()
            .call(&[ch.clone()], &mut ctx)
            .unwrap();
        assert_eq!(got, NonFunctionValue::new(json!(1)));

        assert!(
            module
                .function("send")
                .unwrap()
                .call(&[ch.clone(), NonFunctionValue::new(json!(3))], &mut ctx)
                .is_ok()
        );

        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[ch.clone()], &mut ctx)
                .unwrap(),
            NonFunctionValue::new(json!(2))
        );
        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[ch.clone()], &mut ctx)
                .unwrap(),
            NonFunctionValue::new(json!(3))
        );

        let arg = ch.clone();
        assert_eq!(
            module
                .function("recv")
                .unwrap()
                .call(&[arg.clone()], &mut ctx),
            Err(Error::Operation {
                name: "recv".to_string(),
                args: vec![arg],
            })
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
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "locked!" })));

        let err = module.function("unlock").unwrap().call(&[m], &mut ctx);
        assert!(
            matches!(err, Err(Error::PrimitiveError(_))),
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
            .call(&[NonFunctionValue::new(json!(3))], &mut ctx)
            .unwrap();

        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem.clone(), NonFunctionValue::new(json!(2))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem.clone(), NonFunctionValue::new(json!(2))], &mut ctx)
                .is_err()
        );
        assert!(
            module
                .function("release")
                .unwrap()
                .call(&[sem.clone(), NonFunctionValue::new(json!(2))], &mut ctx)
                .is_ok()
        );
        assert!(
            module
                .function("acquire")
                .unwrap()
                .call(&[sem, NonFunctionValue::new(json!(2))], &mut ctx)
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
            .call(&[NonFunctionValue::new(json!(0))], &mut ctx)
            .unwrap();
        assert_eq!(r.get("type"), Some(&json!("atomic")));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, NonFunctionValue::new(json!(0)));

        let old = module
            .function("atomic_write")
            .unwrap()
            .call(&[r.clone(), NonFunctionValue::new(json!(42))], &mut ctx)
            .unwrap();
        assert_eq!(old, NonFunctionValue::new(json!(0)));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, NonFunctionValue::new(json!(42)));

        let swapped = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r.clone(),
                    NonFunctionValue::new(json!(42)),
                    NonFunctionValue::new(json!(7)),
                ],
                &mut ctx,
            )
            .unwrap();
        assert_eq!(swapped, NonFunctionValue::new(json!(true)));

        let read = module
            .function("atomic_read")
            .unwrap()
            .call(&[r.clone()], &mut ctx)
            .unwrap();
        assert_eq!(read, NonFunctionValue::new(json!(7)));

        let failed = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r,
                    NonFunctionValue::new(json!(42)),
                    NonFunctionValue::new(json!(9)),
                ],
                &mut ctx,
            )
            .unwrap();
        assert_eq!(failed, NonFunctionValue::new(json!(false)));
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
        assert!(matches!(err, Err(Error::PrimitiveError(_))));

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
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "ok" })));
    }

    #[test]
    fn cancel_pending_task() {
        let module = ConcurrencyModule::new();
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

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
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn cancelable_does_not_run_when_already_cancelled() {
        let module = ConcurrencyModule::new();
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

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
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn cancelable_runs_then_result_returned() {
        let module = ConcurrencyModule::new();
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

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
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "ok" })));
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
            .call(&[NonFunctionValue::new(json!(2))], &mut ctx)
            .unwrap();

        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), NonFunctionValue::new(json!(1))], &mut ctx)
            .unwrap();
        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), NonFunctionValue::new(json!(2))], &mut ctx)
            .unwrap();

        let three = NonFunctionValue::new(json!(3));
        let err = module
            .function("try_send")
            .unwrap()
            .call(&[ch.clone(), three.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(Error::Operation {
                name: "try_send".to_string(),
                args: vec![ch, three],
            })
        );
    }

    #[test]
    fn try_acquire_insufficient_blocks() {
        let module = ConcurrencyModule::new();
        let mut ctx = ctx();

        let sem = module
            .function("semaphore")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(2))], &mut ctx)
            .unwrap();

        let three = NonFunctionValue::new(json!(3));
        let err = module
            .function("try_acquire")
            .unwrap()
            .call(&[sem.clone(), three.clone()], &mut ctx);
        assert_eq!(
            err,
            Err(Error::Operation {
                name: "try_acquire".to_string(),
                args: vec![sem, three],
            })
        );
    }

    #[derive(Clone)]
    struct FastEval;

    impl Eval<NonFunctionValue> for FastEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            if expr == "fast" {
                Ok(NonFunctionValue::new(json!(42)))
            } else {
                Ok(NonFunctionValue::new(json!({ "body": expr })))
            }
        }
    }

    #[derive(Clone)]
    struct SlowEval {
        flag: Arc<AtomicBool>,
    }

    impl Eval<NonFunctionValue> for SlowEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            for _ in 0..200 {
                if self.flag.load(Ordering::SeqCst) {
                    return Err(Error::PrimitiveError("cancelled".to_string()));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Ok(NonFunctionValue::new(json!("done")))
        }
    }

    #[test]
    fn sleep_is_a_function() {
        let module = ConcurrencyModule::new();
        assert!(module.function("sleep").is_some());
    }

    #[test]
    fn timeout_returns_result_before_deadline() {
        let scheduler = Arc::new(ThreadPoolScheduler::new(2));
        let module = ConcurrencyModule::with_scheduler(scheduler);
        let mut ctx = CanonicalContext::new();
        ctx.set_sync_eval(Arc::new(FastEval));

        let task = module
            .function("spawn")
            .unwrap()
            .call(&[thunk("fast")], &mut ctx)
            .unwrap();
        let result = module
            .function("timeout")
            .unwrap()
            .call(&[task, NonFunctionValue::new(json!(100))], &mut ctx)
            .unwrap();

        assert_eq!(result, NonFunctionValue::new(json!(42)));
    }

    #[test]
    fn timeout_cancels_slow_task() {
        let cancel = Arc::new(DefaultCancelRuntime::new());
        let scheduler = Arc::new(ThreadPoolScheduler::with_cancel_runtime(2, cancel.clone()));
        let module = ConcurrencyModule::with_full_backends(
            scheduler,
            Arc::new(DefaultSyncRuntime::new()),
            Arc::new(DefaultAtomicRuntime::new()),
            cancel.clone(),
            Arc::new(StdNetIoRuntime::new()),
            Arc::new(ThreadTimerRuntime::new()),
        );
        let mut ctx = CanonicalContext::new();

        let token = module
            .function("cancel_token")
            .unwrap()
            .call(&[], &mut ctx)
            .unwrap();
        let flag = cancel.cancel_flag(&token).unwrap();
        ctx.set_sync_eval(Arc::new(SlowEval { flag }));

        let task = module
            .function("cancelable")
            .unwrap()
            .call(&[token, thunk("slow")], &mut ctx)
            .unwrap();
        let err = module
            .function("timeout")
            .unwrap()
            .call(&[task, NonFunctionValue::new(json!(50))], &mut ctx);

        assert!(matches!(err, Err(Error::PrimitiveError(_))));
    }
}

#[cfg(test)]
mod error_tests;

pub mod plugin;
pub use plugin::LibraryPlugin;
