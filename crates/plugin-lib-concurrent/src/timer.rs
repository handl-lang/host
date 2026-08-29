//! Timer backend for `handl-lib-concurrent`.
//!
//! The [`TimerRuntime`] trait is a **replaceable backend** for the
//! timer primitives used by the schedulers: `sleep` and `timeout`.
//! [`ThreadTimerRuntime`] is the bundled, in-memory, `std`-based default
//! backed by a dedicated worker thread.

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use handl_plugin_vm_values_json::Error;

/// A replaceable backend for timer operations used by the schedulers.
pub trait TimerRuntime: Send + Sync {
    /// Schedule `callback` to run after `delay_ms` milliseconds.
    fn schedule<F>(&self, delay_ms: u64, callback: F)
    where
        Self: Sized,
        F: FnOnce() + Send + 'static,
    {
        self.schedule_boxed(delay_ms, Box::new(callback));
    }

    /// Object-safe version of `schedule` used through `dyn TimerRuntime`.
    fn schedule_boxed(&self, delay_ms: u64, callback: Box<dyn FnOnce() + Send + 'static>);

    /// Block the calling thread until `delay_ms` milliseconds have elapsed.
    fn sleep(&self, delay_ms: u64) -> Result<(), Error>;
}

enum Command {
    Schedule {
        delay_ms: u64,
        callback: Box<dyn FnOnce() + Send + 'static>,
    },
    Sleep {
        delay_ms: u64,
        tx: mpsc::Sender<()>,
    },
    Shutdown,
}

/// A `TimerRuntime` backed by a dedicated worker thread.
///
/// The worker processes one command at a time: it sleeps for the requested
/// delay and then runs the callback or sends a wake-up message.  This is a
/// deliberate intermediate step; it centralises timing on one thread instead
/// of spawning a fresh `std::thread` for every `sleep`/`timeout`.
pub struct ThreadTimerRuntime {
    tx: Mutex<Option<mpsc::Sender<Command>>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl ThreadTimerRuntime {
    /// Creates a new thread-backed timer runtime.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();

        let handle = thread::spawn(move || {
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Command::Shutdown => break,
                    Command::Schedule { delay_ms, callback } => {
                        thread::sleep(Duration::from_millis(delay_ms));
                        callback();
                    }
                    Command::Sleep { delay_ms, tx } => {
                        thread::sleep(Duration::from_millis(delay_ms));
                        let _ = tx.send(());
                    }
                }
            }
        });

        Self {
            tx: Mutex::new(Some(tx)),
            handle: Mutex::new(Some(handle)),
        }
    }

    fn send(&self, cmd: Command) -> Result<(), Error> {
        let guard = self
            .tx
            .lock()
            .map_err(|_| Error::PrimitiveError("timer runtime lock poisoned".to_string()))?;
        if let Some(tx) = guard.as_ref() {
            tx.send(cmd).map_err(|_| {
                Error::PrimitiveError("timer runtime shut down".to_string())
            })
        } else {
            Err(Error::PrimitiveError(
                "timer runtime shut down".to_string(),
            ))
        }
    }
}

impl Default for ThreadTimerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ThreadTimerRuntime {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(Command::Shutdown);
            }
        }
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(h) = guard.take() {
                let _ = h.join();
            }
        }
    }
}

impl TimerRuntime for ThreadTimerRuntime {
    fn schedule_boxed(&self, delay_ms: u64, callback: Box<dyn FnOnce() + Send + 'static>) {
        let _ = self.send(Command::Schedule { delay_ms, callback });
    }

    fn sleep(&self, delay_ms: u64) -> Result<(), Error> {
        let (tx, rx) = mpsc::channel::<()>();
        self.send(Command::Sleep { delay_ms, tx })?;
        rx.recv().map_err(|_| {
            Error::PrimitiveError("timer runtime shut down".to_string())
        })
    }
}

/// A no-op `TimerRuntime` that executes callbacks immediately and `sleep`s
/// using `std::thread::sleep` on the calling thread.
pub struct NoopTimerRuntime;

impl NoopTimerRuntime {
    /// Creates a new no-op timer runtime.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopTimerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerRuntime for NoopTimerRuntime {
    fn schedule_boxed(&self, _delay_ms: u64, callback: Box<dyn FnOnce() + Send + 'static>) {
        callback();
    }

    fn sleep(&self, delay_ms: u64) -> Result<(), Error> {
        thread::sleep(Duration::from_millis(delay_ms));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::Scheduler;
    use handl_plugin_vm_values_json::{CanonicalContext, Error as _Error, Eval, FunctionValue, NonFunctionValue};
    use serde_json::json;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::thread;
    use std::time::{Duration, Instant};

    fn work(body: &str) -> FunctionValue {
        FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: body.to_string(),
            patterns: Vec::new(),
        }
    }

    #[derive(Clone)]
    struct FastEval;

    impl Eval<NonFunctionValue> for FastEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, _Error> {
            Ok(NonFunctionValue::new(json!("done")))
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
        ) -> Result<NonFunctionValue, _Error> {
            for _ in 0..200 {
                if self.flag.load(Ordering::SeqCst) {
                    return Err(_Error::PrimitiveError("cancelled".to_string()));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Ok(NonFunctionValue::new(json!("done")))
        }
    }

    #[test]
    fn thread_timer_runs_callback_after_delay() {
        let timer = ThreadTimerRuntime::new();
        let start = Instant::now();
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();

        timer.schedule(50, move || {
            flag2.store(true, Ordering::SeqCst);
        });

        while !flag.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(45),
            "callback fired too early: {:?}",
            elapsed
        );
    }

    #[test]
    fn sleep_waits_for_delay() {
        let timer = ThreadTimerRuntime::new();
        let start = Instant::now();

        timer.sleep(50).unwrap();

        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(45), "sleep returned too early: {:?}", elapsed);
    }

    #[test]
    fn default_timeout_cancels_after_deadline() {
        let timer = Arc::new(ThreadTimerRuntime::new());
        let cancel = Arc::new(crate::cancel::DefaultCancelRuntime::new());
        let s = crate::scheduler::DefaultScheduler::with_timer(cancel.clone(), timer);

        let token = s.cancel_runtime().cancel_token();
        let flag = s.cancel_runtime().cancel_flag(&token).unwrap();

        let mut ctx = CanonicalContext::new();
        ctx.set_sync_eval(Arc::new(SlowEval { flag }));

        let task = s
            .spawn_cancelable(token.clone(), work("slow"), &mut ctx)
            .unwrap();
        let err = s.timeout(task, 50, &mut ctx);

        assert!(matches!(err, Err(_Error::PrimitiveError(_))));
    }

    #[test]
    fn default_timeout_returns_before_deadline() {
        let timer = Arc::new(ThreadTimerRuntime::new());
        let cancel = Arc::new(crate::cancel::DefaultCancelRuntime::new());
        let s = crate::scheduler::DefaultScheduler::with_timer(cancel, timer);

        let mut ctx = CanonicalContext::new();
        ctx.set_sync_eval(Arc::new(FastEval));

        let task = s.spawn(work("fast"), &mut ctx).unwrap();
        let result = s.timeout(task, 1000, &mut ctx).unwrap();

        assert_eq!(result, NonFunctionValue::new(json!("done")));
    }
}
