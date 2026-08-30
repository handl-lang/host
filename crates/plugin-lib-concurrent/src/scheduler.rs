//! Lego scheduler backend for `handl-lib-concurrent`.
//!
//! The [`Scheduler`] trait is a **replaceable backend** for the three
//! cooperative concurrency primitives exposed by [`ConcurrencyModule`]:
//! `spawn`, `await`, and `yield`. [`DefaultScheduler`] is the bundled,
//! in-memory, single-threaded cooperative scheduler that runs task bodies
//! through `Context::eval_expr`.
//!
//! A host builder can swap the scheduler by providing any `Arc<dyn Scheduler + Send + Sync>`
//! via [`ConcurrencyModule::with_scheduler`](crate::ConcurrencyModule::with_scheduler),
//! making this the first lego piece of the `lib-concurrent` family.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_deque::{Injector, Steal, Stealer, Worker};

use crate::cancel::{CancelRuntime, DefaultCancelRuntime};
use crate::timer::{ThreadTimerRuntime, TimerRuntime};
use handl_platform::{EvalContext, EvalError};
#[cfg(test)]
use handl_platform::ExprEvaluator;
use serde_json::Value;
use crate::function_value::FunctionValue;
use serde_json::json;

/// A replaceable backend for the `concurrent.spawn`, `concurrent.await`,
/// and `concurrent.yield` primitives.
pub trait Scheduler {
    /// Spawn `work` as a new task and return a `Task` value.
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Wait for `task` to finish, running it via `ctx.eval_expr` if needed.
    fn await_task(
        &self,
        task: Value,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Yield control to the scheduler and run one pending task if any.
    fn yield_now(&self, ctx: &mut EvalContext)
    -> Result<Value, EvalError>;

    /// Cancel any pending or running tasks associated with `token`.
    fn cancel(&self, token: Value) -> Result<Value, EvalError> {
        let _ = token;
        Ok(Value::Null)
    }

    /// Spawn `work` as a cancelable task and return a `Task` value.
    fn spawn_cancelable(
        &self,
        token: Value,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = token;
        self.spawn(work, ctx)
    }

    /// Pause execution for `ms` milliseconds.
    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = ms;
        Ok(Value::default())
    }

    /// Wait for `task` to finish or `ms` milliseconds to elapse.
    fn timeout(
        &self,
        task: Value,
        _ms: u64,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.await_task(task, ctx)
    }

    /// Gracefully shut down the scheduler, cancelling pending tasks and
    /// joining worker threads.
    fn shutdown(&self) {
        let _ = self;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl TaskStatus {
    fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }
}

#[derive(Debug)]
struct TaskRecord {
    id: u64,
    work: FunctionValue,
    status: TaskStatus,
    result: Option<Value>,
    error: Option<String>,
    token_id: Option<u64>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

fn task_value(id: u64, status: &str) -> Value {
    serde_json::json!({
        "type": "task",
        "id": id,
        "status": status,
    })
}

fn task_id(task: &Value) -> Result<u64, EvalError> {
    let obj = task
        .as_object()
        .ok_or_else(|| EvalError::Other("task value must be an object".to_string()))?;
    let id = obj
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| EvalError::Other("task value must contain a numeric id".to_string()))?;
    Ok(id)
}

fn cancel_token_id(token: &Value) -> Result<u64, EvalError> {
    let obj = token
        .as_object()
        .ok_or_else(|| EvalError::Other("cancel token must be an object".to_string()))?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EvalError::Other("cancel token missing 'type'".to_string()))?;
    if kind != "cancel_token" {
        return Err(EvalError::Other(format!(
            "expected cancel_token, got {kind}"
        )));
    }
    obj.get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| EvalError::Other("cancel token missing numeric 'id'".to_string()))
}

/// An in-memory, single-threaded cooperative scheduler.
///
/// Tasks are identified by monotonically increasing `u64` ids and are run
/// synchronously when `await_task` or `yield_now` is called. A completed
/// task caches its result so later awaits return without re-executing.
pub struct DefaultScheduler {
    next_id: AtomicU64,
    tasks: Mutex<Vec<TaskRecord>>,
    cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
    timer: Arc<dyn TimerRuntime + Send + Sync>,
}

impl DefaultScheduler {
    /// Creates a new empty scheduler.
    pub fn new() -> Self {
        Self::with_cancel_runtime(Arc::new(DefaultCancelRuntime::new()))
    }

    /// Creates a scheduler that shares a cancellation runtime with the caller.
    pub fn with_cancel_runtime(cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>) -> Self {
        Self::with_timer(cancel_runtime, Arc::new(ThreadTimerRuntime::new()))
    }

    /// Creates a scheduler with a shared cancellation runtime and timer backend.
    pub fn with_timer(
        cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
        timer: Arc<dyn TimerRuntime + Send + Sync>,
    ) -> Self {
        Self {
            next_id: AtomicU64::new(0),
            tasks: Mutex::new(Vec::new()),
            cancel_runtime,
            timer,
        }
    }

    /// Returns the cancellation runtime used by this scheduler.
    pub fn cancel_runtime(&self) -> Arc<dyn CancelRuntime + Send + Sync> {
        self.cancel_runtime.clone()
    }

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<Vec<TaskRecord>>, EvalError> {
        self.tasks
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))
    }
}

impl Default for DefaultScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for DefaultScheduler {
    fn spawn(
        &self,
        work: FunctionValue,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut tasks = self.lock_tasks()?;
        tasks.push(TaskRecord {
            id,
            work,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            token_id: None,
            cancel_flag: None,
        });
        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn spawn_cancelable(
        &self,
        token: Value,
        work: FunctionValue,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(EvalError::Other("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut tasks = self.lock_tasks()?;
        tasks.push(TaskRecord {
            id,
            work,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            token_id: Some(token_id),
            cancel_flag: Some(cancel_flag),
        });
        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn cancel(&self, token: Value) -> Result<Value, EvalError> {
        // Set the token's shared flag so any running task sees the cancellation.
        let _ = self.cancel_runtime.cancel(token.clone())?;
        let token_id = cancel_token_id(&token)?;

        let mut tasks = self.lock_tasks()?;
        for rec in tasks.iter_mut() {
            if rec.token_id == Some(token_id) {
                if rec.status == TaskStatus::Pending || rec.status == TaskStatus::Running {
                    rec.status = TaskStatus::Failed;
                    rec.error = Some("cancelled".to_string());
                }
            }
        }
        Ok(Value::Null)
    }

    fn await_task(
        &self,
        task: Value,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        let (id, body, cancel_flag) = {
            let mut tasks = self.lock_tasks()?;
            let rec = tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| EvalError::Other(format!("task {id} not found")))?;

            match rec.status {
                TaskStatus::Completed => {
                    return Ok(rec.result.clone().unwrap_or_default());
                }
                TaskStatus::Failed => {
                    return Err(EvalError::Other(rec.error.clone().unwrap_or_default()));
                }
                TaskStatus::Running => {
                    return Err(EvalError::Other(format!(
                        "task {id} is already running"
                    )));
                }
                TaskStatus::Pending => {
                    rec.status = TaskStatus::Running;
                    (rec.id, rec.work.body.clone(), rec.cancel_flag.clone())
                }
            }
        };

        if let Some(flag) = &cancel_flag {
            if flag.load(Ordering::SeqCst) {
                let mut tasks = self.lock_tasks()?;
                if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                    rec.status = TaskStatus::Failed;
                    rec.error = Some("cancelled".to_string());
                }
                return Err(EvalError::Other("cancelled".to_string()));
            }
        }

        let res = if false {
            // TODO(ADR 0040 migration): eval_arc not available
            let flag = cancel_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
            let _ = flag;
            Err(EvalError::Other("eval not available".to_string()))
        } else {
            if let Some(flag) = &cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Failed;
                        rec.error = Some("cancelled".to_string());
                    }
                    return Err(EvalError::Other("cancelled".to_string()));
                }
            }
            let res = ctx.eval_expr(&body, &[]);
            if let Some(flag) = &cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Failed;
                        rec.error = Some("cancelled".to_string());
                    }
                    return Err(EvalError::Other("cancelled".to_string()));
                }
            }
            res
        };

        match res {
            Ok(value) => {
                let mut tasks = self.lock_tasks()?;
                if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                    rec.status = TaskStatus::Completed;
                    rec.result = Some(value.clone());
                }
                Ok(value)
            }
            Err(e) => {
                let mut tasks = self.lock_tasks()?;
                if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                    rec.status = TaskStatus::Failed;
                    rec.error = Some(e.to_string());
                }
                Err(e)
            }
        }
    }

    fn yield_now(
        &self,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let maybe = {
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.status == TaskStatus::Pending) {
                rec.status = TaskStatus::Running;
                Some((rec.id, rec.work.body.clone(), rec.cancel_flag.clone()))
            } else {
                None
            }
        };

        if let Some((id, body, cancel_flag)) = maybe {
            if let Some(flag) = &cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Failed;
                        rec.error = Some("cancelled".to_string());
                    }
                    return Err(EvalError::Other("cancelled".to_string()));
                }
            }

            let res = if false {
                // TODO(ADR 0040 migration): eval_arc not available
                let flag = cancel_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
                let _ = flag;
                Err(EvalError::Other("eval not available".to_string()))
            } else {
                if let Some(flag) = &cancel_flag {
                    if flag.load(Ordering::SeqCst) {
                        let mut tasks = self.lock_tasks()?;
                        if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                            rec.status = TaskStatus::Failed;
                            rec.error = Some("cancelled".to_string());
                        }
                        return Err(EvalError::Other("cancelled".to_string()));
                    }
                }
                let res = ctx.eval_expr(&body, &[]);
                if let Some(flag) = &cancel_flag {
                    if flag.load(Ordering::SeqCst) {
                        let mut tasks = self.lock_tasks()?;
                        if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                            rec.status = TaskStatus::Failed;
                            rec.error = Some("cancelled".to_string());
                        }
                        return Err(EvalError::Other("cancelled".to_string()));
                    }
                }
                res
            };

            match res {
                Ok(value) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Completed;
                        rec.result = Some(value);
                    }
                }
                Err(e) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Failed;
                        rec.error = Some(e.to_string());
                    }
                    return Err(e);
                }
            }
        }

        Ok(Value::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.timer.sleep(ms)?;
        Ok(Value::default())
    }

    fn timeout(
        &self,
        task: Value,
        ms: u64,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            let rec = tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| EvalError::Other(format!("task {id} not found")))?;

            if let Some(token_id) = rec.token_id {
                serde_json::json!(json!({
                    "type": "cancel_token",
                    "id": token_id,
                    "cancelled": false,
                }))
            } else {
                let token = self.cancel_runtime.cancel_token();
                let token_id = cancel_token_id(&token)?;
                rec.token_id = Some(token_id);
                token
            }
        };

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        {
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.cancel_flag = Some(cancel_flag.clone());
            }
        }

        let flag = cancel_flag;
        self.timer.schedule_boxed(
            ms,
            Box::new(move || {
                flag.store(true, Ordering::SeqCst);
            }),
        );

        self.await_task(task, ctx)
    }
}

/// A unit of work sent to a worker thread.
enum Job {
    Execute {
        id: u64,
        // `body` is currently never read because task bodies cannot be evaluated
        // after the ADR 0040 migration (ExprEvaluator is not Send + Sync); the
        // field is retained for the future restoration of threaded evaluation.
        #[allow(dead_code)]
        body: String,
        cancel_flag: Option<Arc<AtomicBool>>,
    },
    Shutdown,
}

type EvalResult = Result<Value, String>;

fn worker_thread(jobs: Arc<Mutex<mpsc::Receiver<Job>>>, results: mpsc::Sender<(u64, EvalResult)>) {
    while let Ok(job) = jobs.lock().unwrap().recv() {
        match job {
            Job::Shutdown => break,
            Job::Execute {
                id,
                body: _,
                cancel_flag,
            } => {
                // TODO(ADR 0040 migration): The old system used Arc<dyn Eval + Send + Sync>
                // to evaluate task bodies in worker threads. The new ExprEvaluator trait
                // is not Send + Sync, so it cannot be used in threaded schedulers.
                // For now, tasks in ThreadPoolScheduler cannot be evaluated.
                let _ = cancel_flag;
                let res = Err(
                    "ThreadPoolScheduler does not support task evaluation after ADR 0040 migration".to_string(),
                );

                // The receiver may have been dropped; ignore send errors.
                let _ = results.send((id, res));
            }
        }
    }
}

/// An OS-thread pool scheduler.
///
/// Tasks are distributed to worker threads. Each task's body is evaluated
/// by a `Send + Sync` [`Eval`] callback obtained from the caller's
/// [`Context::eval_arc`]. This requires the context to have a thread-safe
/// eval callback set (e.g. `CanonicalContext::set_sync_eval`); otherwise
/// `spawn` returns an error.
pub struct ThreadPoolScheduler {
    next_id: AtomicU64,
    jobs_tx: Mutex<Option<mpsc::Sender<Job>>>,
    results_rx: Mutex<mpsc::Receiver<(u64, EvalResult)>>,
    tasks: Mutex<Vec<TaskRecord>>,
    cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
    timer: Arc<dyn TimerRuntime + Send + Sync>,
    shutdown: AtomicBool,
    worker_handles: Mutex<Option<Vec<thread::JoinHandle<()>>>>,
}

impl ThreadPoolScheduler {
    /// Creates a new thread-pool scheduler with the given number of workers.
    pub fn new(workers: usize) -> Self {
        Self::with_cancel_runtime(workers, Arc::new(DefaultCancelRuntime::new()))
    }

    /// Creates a new thread-pool scheduler that shares a cancellation runtime.
    pub fn with_cancel_runtime(
        workers: usize,
        cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
    ) -> Self {
        Self::with_timer(workers, cancel_runtime, Arc::new(ThreadTimerRuntime::new()))
    }

    /// Creates a new thread-pool scheduler with a shared timer backend.
    pub fn with_timer(
        workers: usize,
        cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
        timer: Arc<dyn TimerRuntime + Send + Sync>,
    ) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let jobs_rx = Arc::new(Mutex::new(jobs_rx));
        let (results_tx, results_rx) = mpsc::channel();

        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let jobs = jobs_rx.clone();
            let results = results_tx.clone();
            handles.push(thread::spawn(move || worker_thread(jobs, results)));
        }

        Self {
            next_id: AtomicU64::new(0),
            jobs_tx: Mutex::new(Some(jobs_tx)),
            results_rx: Mutex::new(results_rx),
            tasks: Mutex::new(Vec::new()),
            cancel_runtime,
            timer,
            shutdown: AtomicBool::new(false),
            worker_handles: Mutex::new(Some(handles)),
        }
    }

    /// Returns the cancellation runtime used by this scheduler.
    pub fn cancel_runtime(&self) -> Arc<dyn CancelRuntime + Send + Sync> {
        self.cancel_runtime.clone()
    }

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<Vec<TaskRecord>>, EvalError> {
        self.tasks
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))
    }

    fn lock_results(
        &self,
    ) -> Result<std::sync::MutexGuard<mpsc::Receiver<(u64, EvalResult)>>, EvalError> {
        self.results_rx
            .lock()
            .map_err(|_| EvalError::Other("result channel lock poisoned".to_string()))
    }

    fn process_results(&self) -> Result<Value, EvalError> {
        let results = self.lock_results()?;
        loop {
            match results.try_recv() {
                Ok((id, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        if rec.status == TaskStatus::Pending || rec.status == TaskStatus::Running {
                            match res {
                                Ok(value) => {
                                    rec.status = TaskStatus::Completed;
                                    rec.result = Some(value);
                                }
                                Err(e) => {
                                    rec.status = TaskStatus::Failed;
                                    rec.error = Some(e);
                                }
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break Ok(Value::Null),
                Err(mpsc::TryRecvError::Disconnected) => break Ok(Value::Null),
            }
        }
    }

    /// Gracefully shut down the pool.
    ///
    /// Signals each worker to stop, closes the job channel, marks any
    /// still-pending tasks as failed, and joins each worker handle with a
    /// one-second timeout.  Returns a vector of booleans indicating whether
    /// each worker was joined before the timeout.  Calling this more than
    /// once is a no-op.
    pub fn shutdown(&self) -> Result<Value, EvalError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(Value::Null);
        }

        let worker_count = {
            let guard = self
                .worker_handles
                .lock()
                .map_err(|_| EvalError::Other("worker handles lock poisoned".to_string()))?;
            guard.as_ref().map(|v| v.len()).unwrap_or(0)
        };

        if worker_count == 0 {
            return Ok(Value::Null);
        }

        // Queue a shutdown job for each worker and close the job sender.
        let sender = self
            .jobs_tx
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))?
            .take();
        if let Some(tx) = sender {
            for _ in 0..worker_count {
                if tx.send(Job::Shutdown).is_err() {
                    break;
                }
            }
        }

        // Mark pending tasks as failed so that awaiting them does not hang.
        {
            let mut tasks = self.lock_tasks()?;
            for rec in tasks.iter_mut() {
                if rec.status == TaskStatus::Pending {
                    rec.status = TaskStatus::Failed;
                    rec.error = Some("scheduler shut down".to_string());
                }
            }
        }

        // Join each worker with a one-second timeout.  We do the join in a
        // helper thread so that `shutdown` itself cannot hang if a worker is
        // stuck.
        let handles = {
            let mut guard = self
                .worker_handles
                .lock()
                .map_err(|_| EvalError::Other("worker handles lock poisoned".to_string()))?;
            guard.take().unwrap_or_default()
        };

        let mut joined = Vec::with_capacity(handles.len());
        for handle in handles {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => joined.push(true),
                Err(_) => joined.push(false),
            }
        }

        Ok(serde_json::json!(joined))
    }
}

impl Drop for ThreadPoolScheduler {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::SeqCst) {
            let _ = ThreadPoolScheduler::shutdown(self);
        }
    }
}

impl Default for ThreadPoolScheduler {
    fn default() -> Self {
        Self::new(2)
    }
}

impl Scheduler for ThreadPoolScheduler {
    // The body after the early `return Err(...)` is unreachable pending the
    // restoration of Send + Sync evaluation after the ADR 0040 migration; the
    // implementation is retained for that future work.
    #[allow(unreachable_code, unused_variables)]
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = ctx; // TODO: eval_arc not available after ADR 0040
        return Err(EvalError::Other("ThreadPoolScheduler requires a Send + Sync eval callback; \
                 set one with CanonicalContext::set_sync_eval".to_string()));

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let body = work.body.clone();
        let mut tasks = self.lock_tasks()?;
        tasks.push(TaskRecord {
            id,
            work,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            token_id: None,
            cancel_flag: None,
        });
        drop(tasks);

        let guard = self
            .jobs_tx
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))?;
        let tx = guard
            .as_ref()
            .ok_or_else(|| EvalError::Other("scheduler shut down".to_string()))?;
        if tx
            .send(Job::Execute {
                id,
                body,
                cancel_flag: None,
            })
            .is_err()
        {
            drop(guard);
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.status = TaskStatus::Failed;
                rec.error = Some("scheduler shut down".to_string());
            }
            return Err(EvalError::Other("scheduler shut down".to_string()));
        }

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    #[allow(unreachable_code, unused_variables)]
    fn spawn_cancelable(
        &self,
        token: Value,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = ctx; // TODO: eval_arc not available after ADR 0040
        return Err(EvalError::Other("ThreadPoolScheduler requires a Send + Sync eval callback; \
                 set one with CanonicalContext::set_sync_eval".to_string()));

        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(EvalError::Other("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let body = work.body.clone();
        let mut tasks = self.lock_tasks()?;
        tasks.push(TaskRecord {
            id,
            work,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            token_id: Some(token_id),
            cancel_flag: Some(cancel_flag.clone()),
        });
        drop(tasks);

        let guard = self
            .jobs_tx
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))?;
        let tx = guard
            .as_ref()
            .ok_or_else(|| EvalError::Other("scheduler shut down".to_string()))?;
        if tx
            .send(Job::Execute {
                id,
                body,
                cancel_flag: Some(cancel_flag),
            })
            .is_err()
        {
            drop(guard);
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.status = TaskStatus::Failed;
                rec.error = Some("scheduler shut down".to_string());
            }
            return Err(EvalError::Other("scheduler shut down".to_string()));
        }

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn cancel(&self, token: Value) -> Result<Value, EvalError> {
        let _ = self.cancel_runtime.cancel(token.clone())?;
        let token_id = cancel_token_id(&token)?;

        let mut tasks = self.lock_tasks()?;
        for rec in tasks.iter_mut() {
            if rec.token_id == Some(token_id) {
                if rec.status == TaskStatus::Pending || rec.status == TaskStatus::Running {
                    rec.status = TaskStatus::Failed;
                    rec.error = Some("cancelled".to_string());
                }
            }
        }
        Ok(Value::Null)
    }

    fn await_task(
        &self,
        task: Value,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        loop {
            self.process_results()?;

            let running = {
                let mut tasks = self.lock_tasks()?;
                if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                    match rec.status {
                        TaskStatus::Completed => {
                            return Ok(rec.result.clone().unwrap_or_default());
                        }
                        TaskStatus::Failed => {
                            return Err(EvalError::Other(
                                rec.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            rec.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(EvalError::Other(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            // Wait for a result, then re-process.
            let (rid, res) = self
                .lock_results()?
                .recv()
                .map_err(|_| EvalError::Other("result channel closed".to_string()))?;

            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == rid) {
                if rec.status == TaskStatus::Pending || rec.status == TaskStatus::Running {
                    match res {
                        Ok(value) => {
                            rec.status = TaskStatus::Completed;
                            rec.result = Some(value);
                        }
                        Err(e) => {
                            rec.status = TaskStatus::Failed;
                            rec.error = Some(e);
                        }
                    }
                }
            }
        }
    }

    fn yield_now(
        &self,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.process_results()?;
        Ok(Value::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.timer.sleep(ms)?;
        Ok(Value::default())
    }

    fn timeout(
        &self,
        task: Value,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                if let Some(token_id) = rec.token_id {
                    serde_json::json!(json!({
                        "type": "cancel_token",
                        "id": token_id,
                        "cancelled": false,
                    }))
                } else {
                    let token = self.cancel_runtime.cancel_token();
                    let token_id = cancel_token_id(&token)?;
                    rec.token_id = Some(token_id);
                    token
                }
            } else {
                return Err(EvalError::Other(format!("task {id} not found")));
            }
        };

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        let (timeout_tx, timeout_rx) = mpsc::channel();
        let flag = cancel_flag;
        self.timer.schedule_boxed(
            ms,
            Box::new(move || {
                flag.store(true, Ordering::SeqCst);
                let _ = timeout_tx.send(());
            }),
        );

        loop {
            self.process_results()?;

            let running = {
                let mut tasks = self.lock_tasks()?;
                if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                    match rec.status {
                        TaskStatus::Completed => {
                            return Ok(rec.result.clone().unwrap_or_default());
                        }
                        TaskStatus::Failed => {
                            return Err(EvalError::Other(
                                rec.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            rec.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(EvalError::Other(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            if timeout_rx.try_recv().is_ok() {
                let _ = self.cancel(token.clone())?;
                continue;
            }

            match self.lock_results()?.recv_timeout(Duration::from_millis(1)) {
                Ok((rid, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == rid) {
                        if rec.status == TaskStatus::Pending || rec.status == TaskStatus::Running {
                            match res {
                                Ok(value) => {
                                    rec.status = TaskStatus::Completed;
                                    rec.result = Some(value);
                                }
                                Err(e) => {
                                    rec.status = TaskStatus::Failed;
                                    rec.error = Some(e.to_string());
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(EvalError::Other("result channel closed".to_string()));
                }
            }
        }
    }

    fn shutdown(&self) {
        let _ = ThreadPoolScheduler::shutdown(self);
    }
}

/// Result of executing a `WorkStealingJob`.
///
/// `Ok` and `Err` are currently never constructed because task bodies cannot be
/// evaluated after the ADR 0040 migration (ExprEvaluator is not Send + Sync);
/// only `Cancelled` is sent. The variants are retained for the future restoration
/// of threaded evaluation.
#[allow(dead_code)]
enum TaskResult {
    Ok(Value),
    Err(String),
    Cancelled,
}

/// Per-task state for the work-stealing scheduler.
struct TaskState {
    status: TaskStatus,
    result: Option<Value>,
    error: Option<String>,
    token_id: Option<u64>,
}

/// A unit of work for the work-stealing thread pool.
enum WsJob {
    Execute {
        id: u64,
        body: String,
        cancel_flag: Arc<AtomicBool>,
        result_tx: mpsc::Sender<(u64, TaskResult)>,
    },
    Shutdown,
}

fn ws_worker(
    injector: Arc<Injector<WsJob>>,
    local: Worker<WsJob>,
    stealers: Vec<Stealer<WsJob>>,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        let mut job = local.pop();

        if job.is_none() {
            job = match injector.steal() {
                Steal::Success(j) => Some(j),
                _ => None,
            };
        }

        if job.is_none() {
            for stealer in &stealers {
                if let Steal::Success(j) = stealer.steal() {
                    job = Some(j);
                    break;
                }
            }
        }

        match job {
            Some(WsJob::Shutdown) => break,
            Some(WsJob::Execute {
                id,
                body,
                cancel_flag,
                result_tx,
            }) => {
                // TODO(ADR 0040 migration): ExprEvaluator is not Send + Sync,
                // so it cannot be used in worker threads.
                let _ = cancel_flag;
                let _ = body;
                let _ = result_tx.send((id, TaskResult::Cancelled));
            }
            None => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                thread::yield_now();
                thread::sleep(Duration::from_nanos(100));
            }
        }
    }
}

/// A work-stealing OS-thread pool scheduler.
pub struct WorkStealingScheduler {
    next_id: AtomicU64,
    pub n_workers: usize,
    injector: Arc<Injector<WsJob>>,
    results_tx: mpsc::Sender<(u64, TaskResult)>,
    results_rx: Mutex<mpsc::Receiver<(u64, TaskResult)>>,
    tasks: Mutex<HashMap<u64, TaskState>>,
    cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
    timer: Arc<dyn TimerRuntime + Send + Sync>,
    shutdown: Arc<AtomicBool>,
    worker_handles: Mutex<Option<Vec<thread::JoinHandle<()>>>>,
}

impl WorkStealingScheduler {
    /// Creates a new work-stealing scheduler with the given number of workers.
    pub fn new(workers: usize) -> Self {
        Self::with_cancel_runtime(workers, Arc::new(DefaultCancelRuntime::new()))
    }

    /// Creates a new work-stealing scheduler that shares a cancellation runtime.
    pub fn with_cancel_runtime(
        workers: usize,
        cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
    ) -> Self {
        Self::with_timer(workers, cancel_runtime, Arc::new(ThreadTimerRuntime::new()))
    }

    /// Creates a new work-stealing scheduler with a shared timer backend.
    pub fn with_timer(
        workers: usize,
        cancel_runtime: Arc<dyn CancelRuntime + Send + Sync>,
        timer: Arc<dyn TimerRuntime + Send + Sync>,
    ) -> Self {
        let injector: Arc<Injector<WsJob>> = Arc::new(Injector::new());
        let (results_tx, results_rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));

        let worker_queue: Vec<Worker<WsJob>> = (0..workers).map(|_| Worker::new_fifo()).collect();
        let all_stealers: Vec<Stealer<WsJob>> = worker_queue.iter().map(|w| w.stealer()).collect();

        let mut handles = Vec::with_capacity(workers);
        for (i, local) in worker_queue.into_iter().enumerate() {
            let stealers: Vec<_> = all_stealers
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, s)| s.clone())
                .collect();
            let injector = injector.clone();
            let shutdown = shutdown.clone();
            handles.push(thread::spawn(move || {
                ws_worker(injector, local, stealers, shutdown)
            }));
        }

        Self {
            next_id: AtomicU64::new(0),
            n_workers: workers,
            injector,
            results_tx,
            results_rx: Mutex::new(results_rx),
            tasks: Mutex::new(HashMap::new()),
            cancel_runtime,
            timer,
            shutdown,
            worker_handles: Mutex::new(Some(handles)),
        }
    }

    /// Returns the cancellation runtime used by this scheduler.
    pub fn cancel_runtime(&self) -> Arc<dyn CancelRuntime + Send + Sync> {
        self.cancel_runtime.clone()
    }

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<HashMap<u64, TaskState>>, EvalError> {
        self.tasks
            .lock()
            .map_err(|_| EvalError::Other("scheduler lock poisoned".to_string()))
    }

    fn lock_results(
        &self,
    ) -> Result<std::sync::MutexGuard<mpsc::Receiver<(u64, TaskResult)>>, EvalError> {
        self.results_rx
            .lock()
            .map_err(|_| EvalError::Other("result channel lock poisoned".to_string()))
    }

    fn process_results(&self) -> Result<Value, EvalError> {
        let results = self.lock_results()?;
        loop {
            match results.try_recv() {
                Ok((id, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(state) = tasks.get_mut(&id) {
                        if state.status == TaskStatus::Pending
                            || state.status == TaskStatus::Running
                        {
                            match res {
                                TaskResult::Ok(value) => {
                                    state.status = TaskStatus::Completed;
                                    state.result = Some(value);
                                }
                                TaskResult::Err(e) => {
                                    state.status = TaskStatus::Failed;
                                    state.error = Some(e);
                                }
                                TaskResult::Cancelled => {
                                    state.status = TaskStatus::Failed;
                                    state.error = Some("cancelled".to_string());
                                }
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break Ok(Value::Null),
                Err(mpsc::TryRecvError::Disconnected) => break Ok(Value::Null),
            }
        }
    }

    /// Gracefully shut down the work-stealing pool.
    pub fn shutdown(&self) -> Result<Value, EvalError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(Value::Null);
        }

        let worker_count = {
            let guard = self
                .worker_handles
                .lock()
                .map_err(|_| EvalError::Other("worker handles lock poisoned".to_string()))?;
            guard.as_ref().map(|v| v.len()).unwrap_or(0)
        };

        if worker_count == 0 {
            return Ok(Value::Null);
        }

        {
            let mut tasks = self.lock_tasks()?;
            for state in tasks.values_mut() {
                if state.status == TaskStatus::Pending {
                    state.status = TaskStatus::Failed;
                    state.error = Some("scheduler shut down".to_string());
                }
            }
        }

        for _ in 0..worker_count {
            self.injector.push(WsJob::Shutdown);
        }

        let handles = {
            let mut guard = self
                .worker_handles
                .lock()
                .map_err(|_| EvalError::Other("worker handles lock poisoned".to_string()))?;
            guard.take().unwrap_or_default()
        };

        let mut joined = Vec::with_capacity(handles.len());
        for handle in handles {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => joined.push(true),
                Err(_) => joined.push(false),
            }
        }

        Ok(serde_json::json!(joined))
    }
}

impl Drop for WorkStealingScheduler {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::SeqCst) {
            let _ = WorkStealingScheduler::shutdown(self);
        }
    }
}

impl Default for WorkStealingScheduler {
    fn default() -> Self {
        Self::new(2)
    }
}

impl Scheduler for WorkStealingScheduler {
    // The body after the early `return Err(...)` is unreachable pending the
    // restoration of Send + Sync evaluation after the ADR 0040 migration; the
    // implementation is retained for that future work.
    #[allow(unreachable_code, unused_variables)]
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = ctx; // TODO: eval_arc not available after ADR 0040
        return Err(EvalError::Other("WorkStealingScheduler requires a Send + Sync eval callback; \
                     set one with CanonicalContext::set_sync_eval".to_string()));

        if self.shutdown.load(Ordering::SeqCst) {
            return Err(EvalError::Other("scheduler shut down".to_string()));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let body = work.body.clone();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let mut tasks = self.lock_tasks()?;
        tasks.insert(
            id,
            TaskState {
                status: TaskStatus::Pending,
                result: None,
                error: None,
                token_id: None,
            },
        );
        drop(tasks);

        self.injector.push(WsJob::Execute {
            id,
            body,
            cancel_flag,
            result_tx: self.results_tx.clone(),
        });

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    #[allow(unreachable_code, unused_variables)]
    fn spawn_cancelable(
        &self,
        token: Value,
        work: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let _ = ctx; // TODO: eval_arc not available after ADR 0040
        return Err(EvalError::Other("WorkStealingScheduler requires a Send + Sync eval callback; \
                     set one with CanonicalContext::set_sync_eval".to_string()));

        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(EvalError::Other("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let body = work.body.clone();

        let mut tasks = self.lock_tasks()?;
        tasks.insert(
            id,
            TaskState {
                status: TaskStatus::Pending,
                result: None,
                error: None,
                token_id: Some(token_id),
            },
        );
        drop(tasks);

        self.injector.push(WsJob::Execute {
            id,
            body,
            cancel_flag,
            result_tx: self.results_tx.clone(),
        });

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn cancel(&self, token: Value) -> Result<Value, EvalError> {
        let _ = self.cancel_runtime.cancel(token.clone())?;
        let token_id = cancel_token_id(&token)?;

        let mut tasks = self.lock_tasks()?;
        for state in tasks.values_mut() {
            if state.token_id == Some(token_id) {
                if state.status == TaskStatus::Pending || state.status == TaskStatus::Running {
                    state.status = TaskStatus::Failed;
                    state.error = Some("cancelled".to_string());
                }
            }
        }
        Ok(Value::Null)
    }

    fn await_task(
        &self,
        task: Value,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        loop {
            self.process_results()?;

            let running = {
                let mut tasks = self.lock_tasks()?;
                if let Some(state) = tasks.get_mut(&id) {
                    match state.status {
                        TaskStatus::Completed => {
                            return Ok(state.result.clone().unwrap_or_default());
                        }
                        TaskStatus::Failed => {
                            return Err(EvalError::Other(
                                state.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            state.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(EvalError::Other(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            let (rid, res) = self
                .lock_results()?
                .recv()
                .map_err(|_| EvalError::Other("result channel closed".to_string()))?;

            let mut tasks = self.lock_tasks()?;
            if let Some(state) = tasks.get_mut(&rid) {
                if state.status == TaskStatus::Pending || state.status == TaskStatus::Running {
                    match res {
                        TaskResult::Ok(value) => {
                            state.status = TaskStatus::Completed;
                            state.result = Some(value);
                        }
                        TaskResult::Err(e) => {
                            state.status = TaskStatus::Failed;
                            state.error = Some(e);
                        }
                        TaskResult::Cancelled => {
                            state.status = TaskStatus::Failed;
                            state.error = Some("cancelled".to_string());
                        }
                    }
                }
            }
        }
    }

    fn yield_now(
        &self,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.process_results()?;

        let job = match self.injector.steal() {
            Steal::Success(j) => j,
            _ => return Ok(Value::default()),
        };

        match job {
            WsJob::Shutdown => {
                self.injector.push(WsJob::Shutdown);
            }
            WsJob::Execute {
                id,
                body,
                cancel_flag,
                result_tx,
            } => {
                // TODO(ADR 0040 migration): ExprEvaluator is not Send + Sync
                let _ = cancel_flag;
                let _ = body;
                let _ = result_tx.send((id, TaskResult::Cancelled));
                self.process_results()?;
            }
        }

        Ok(Value::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        self.timer.sleep(ms)?;
        Ok(Value::default())
    }

    fn timeout(
        &self,
        task: Value,
        ms: u64,
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            if let Some(state) = tasks.get_mut(&id) {
                if let Some(token_id) = state.token_id {
                    serde_json::json!(json!({
                        "type": "cancel_token",
                        "id": token_id,
                        "cancelled": false,
                    }))
                } else {
                    let token = self.cancel_runtime.cancel_token();
                    let token_id = cancel_token_id(&token)?;
                    state.token_id = Some(token_id);
                    token
                }
            } else {
                return Err(EvalError::Other(format!("task {id} not found")));
            }
        };

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| EvalError::Other("cancel token not found".to_string()))?;

        let (timeout_tx, timeout_rx) = mpsc::channel();
        let flag = cancel_flag;
        self.timer.schedule_boxed(
            ms,
            Box::new(move || {
                flag.store(true, Ordering::SeqCst);
                let _ = timeout_tx.send(());
            }),
        );

        loop {
            self.process_results()?;

            let running = {
                let mut tasks = self.lock_tasks()?;
                if let Some(state) = tasks.get_mut(&id) {
                    match state.status {
                        TaskStatus::Completed => {
                            return Ok(state.result.clone().unwrap_or_default());
                        }
                        TaskStatus::Failed => {
                            return Err(EvalError::Other(
                                state.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            state.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(EvalError::Other(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            if timeout_rx.try_recv().is_ok() {
                let _ = self.cancel(token.clone())?;
                continue;
            }

            match self.lock_results()?.recv_timeout(Duration::from_millis(1)) {
                Ok((rid, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(state) = tasks.get_mut(&rid) {
                        if state.status == TaskStatus::Pending
                            || state.status == TaskStatus::Running
                        {
                            match res {
                                TaskResult::Ok(value) => {
                                    state.status = TaskStatus::Completed;
                                    state.result = Some(value);
                                }
                                TaskResult::Err(e) => {
                                    state.status = TaskStatus::Failed;
                                    state.error = Some(e);
                                }
                                TaskResult::Cancelled => {
                                    state.status = TaskStatus::Failed;
                                    state.error = Some("cancelled".to_string());
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(EvalError::Other("result channel closed".to_string()));
                }
            }
        }
    }

    fn shutdown(&self) {
        let _ = WorkStealingScheduler::shutdown(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::EvalContext;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::rc::Rc;
    use crate::cancel::CancelableEval;

    #[derive(Clone, Default)]
    struct MockEval {
        calls: Arc<Mutex<Vec<String>>>,
        cancel: Option<Arc<AtomicBool>>,
    }

    impl ExprEvaluator for MockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            if let Some(flag) = &self.cancel {
                if flag.load(Ordering::SeqCst) {
                    return Err(EvalError::Other("cancelled".to_string()));
                }
            }
            self.calls.lock().unwrap().push(expr.to_string());
            Ok(serde_json::json!(json!({ "body": expr })))
        }
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
    fn spawn_returns_task_value() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn await_runs_task_body() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();
        let result = s.await_task(task, &mut ctx).unwrap();

        assert_eq!(result, serde_json::json!(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);
    }

    #[test]
    fn yield_now_runs_a_pending_task() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("work()"), &mut ctx).unwrap();

        let _ = s.yield_now(&mut ctx).unwrap();
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["work()"]);

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "work()" })));
        assert_eq!(eval.calls.lock().unwrap().len(), 1);
    }

    #[test]
    #[ignore = "ThreadPoolScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn thread_pool_scheduler_spawns_and_awaits() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);
    }

    #[test]
    fn default_cancel_pending_prevents_execution() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = DefaultScheduler::new();
        let token = s.cancel_runtime().cancel_token();
        let task = s
            .spawn_cancelable(token.clone(), work("hello()"), &mut ctx)
            .unwrap();

        s.cancel(token).unwrap();

        let err = s.await_task(task, &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn default_cancelable_already_cancelled() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = DefaultScheduler::new();
        let token = s.cancel_runtime().cancel_token();
        s.cancel_runtime().cancel(token.clone()).unwrap();

        let err = s.spawn_cancelable(token, work("hello()"), &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    #[ignore = "flaky race; see thread pool scheduler"]
    fn thread_pool_cancel_pending_prevents_execution() {
        let s = ThreadPoolScheduler::new(1);
        let token = s.cancel_runtime().cancel_token();
        let flag = s.cancel_runtime().cancel_flag(&token).unwrap();

        let mut ctx = EvalContext::new();
        let eval = MockEval {
            cancel: Some(flag),
            ..Default::default()
        };
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let task = s
            .spawn_cancelable(token.clone(), work("hello()"), &mut ctx)
            .unwrap();

        s.cancel(token).unwrap();

        let err = s.await_task(task, &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn thread_pool_cancelable_already_cancelled() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let token = s.cancel_runtime().cancel_token();
        s.cancel_runtime().cancel(token.clone()).unwrap();

        let err = s.spawn_cancelable(token, work("hello()"), &mut ctx);
        assert!(matches!(err, Err(EvalError::Other(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    #[ignore = "ThreadPoolScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn thread_pool_shutdown_joins_workers() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();
        let result = s.await_task(task, &mut ctx).unwrap();

        assert_eq!(result, serde_json::json!(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);

        let joined = s.shutdown().unwrap();
        let joined_arr = joined.as_array().unwrap();
        assert_eq!(joined_arr.len(), 2);
        assert!(joined_arr.iter().all(|j| j.as_bool().unwrap_or(false)), "all workers should join");
    }

    #[test]
    #[ignore = "ThreadPoolScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn thread_pool_shutdown_cancels_pending() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        // Shut down immediately. The task may or may not have started.
        let joined = s.shutdown().unwrap();
        let joined_arr = joined.as_array().unwrap();
        assert_eq!(joined_arr.len(), 2);

        let err = s.await_task(task, &mut ctx).unwrap_err();
        assert!(
            matches!(err, EvalError::Other(ref msg) if msg == "scheduler shut down"),
            "expected shutdown error, got {err:?}"
        );
    }

    #[test]
    fn thread_pool_scheduler_drops_cleanly() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        {
            let s = ThreadPoolScheduler::new(2);
            let _ = s.spawn(work("hello()"), &mut ctx);
            // Dropping `s` should shut down the pool without panicking.
        }

        // Reaching this point means the scheduler dropped without hanging.
        let _ = eval;
    }

    #[test]
    #[ignore = "WorkStealingScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn work_stealing_spawn_and_await() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = WorkStealingScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "hello()" })));
        assert!(eval.calls.lock().unwrap().contains(&"hello()".to_string()));

        let _ = s.shutdown();
    }

    #[test]
    #[ignore = "WorkStealingScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn work_stealing_spawn_many() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = WorkStealingScheduler::new(4);
        let mut tasks = Vec::with_capacity(100);
        let mut bodies = Vec::with_capacity(100);
        for i in 0..100 {
            let body = format!("task{i}");
            tasks.push(s.spawn(work(&body), &mut ctx).unwrap());
            bodies.push(body);
        }

        for (i, task) in tasks.iter().enumerate() {
            let result = s.await_task(task.clone(), &mut ctx).unwrap();
            assert_eq!(result, serde_json::json!(json!({ "body": bodies[i] })));
        }

        assert_eq!(eval.calls.lock().unwrap().len(), 100);
        let _ = s.shutdown();
    }

    #[test]
    #[ignore = "WorkStealingScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn work_stealing_cancel_pending() {
        let s = WorkStealingScheduler::new(2);
        let token = s.cancel_runtime().cancel_token();

        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let task1 = s
            .spawn_cancelable(token.clone(), work("one()"), &mut ctx)
            .unwrap();
        let task2 = s
            .spawn_cancelable(token.clone(), work("two()"), &mut ctx)
            .unwrap();

        s.cancel(token).unwrap();

        assert!(s.await_task(task1, &mut ctx).is_err());
        assert!(s.await_task(task2, &mut ctx).is_err());

        let _ = s.shutdown();
    }

    #[test]
    #[ignore = "WorkStealingScheduler cannot evaluate tasks after ADR 0040 migration"]
    fn work_stealing_shutdown() {
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let s = WorkStealingScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        let joined = s.shutdown().unwrap();
        let joined_arr = joined.as_array().unwrap();
        assert_eq!(joined_arr.len(), 2);

        let err = s.await_task(task, &mut ctx).unwrap_err();
        assert!(
            matches!(err, EvalError::Other(ref msg) if msg == "scheduler shut down"),
            "expected shutdown error, got {err:?}"
        );
    }

    struct NestedMockEval {
        calls: Arc<Mutex<Vec<String>>>,
        flag: Arc<AtomicBool>,
        wrapped: Rc<Mutex<Option<Rc<CancelableEval>>>>,
    }

    impl NestedMockEval {
        fn new(flag: Arc<AtomicBool>) -> Rc<Self> {
            Rc::new(Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                flag,
                wrapped: Rc::new(Mutex::new(None)),
            })
        }

        fn set_wrapped(&self, wrapped: Rc<CancelableEval>) {
            *self.wrapped.lock().unwrap() = Some(wrapped);
        }
    }

    impl ExprEvaluator for NestedMockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            self.calls.lock().unwrap().push(expr.to_string());
            let call = self.calls.lock().unwrap().len();

            if call == 1 {
                let wrapped = self.wrapped.lock().unwrap().clone().unwrap();
                wrapped.eval_expr("first_nested", &[])?;
                unreachable!("first_nested should return cancelled");
            } else if call == 2 {
                self.flag.store(true, Ordering::SeqCst);
                let wrapped = self.wrapped.lock().unwrap().clone().unwrap();
                wrapped.eval_expr("second_nested", &[])
            } else {
                panic!("unexpected nested call: {expr}");
            }
        }
    }

    #[test]
    fn default_scheduler_cancel_during_execution() {
        let s = DefaultScheduler::new();
        let token = s.cancel_runtime().cancel_token();
        let flag = s.cancel_runtime().cancel_flag(&token).unwrap();

        let inner = NestedMockEval::new(flag.clone());
        let wrapped = Rc::new(CancelableEval::new(inner.clone(), flag.clone()));
        inner.set_wrapped(wrapped.clone());

        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(wrapped);

        let task = s.spawn_cancelable(token, work("body"), &mut ctx).unwrap();
        let err = s.await_task(task, &mut ctx).unwrap_err();

        assert!(
            matches!(err, EvalError::Other(ref m) if m == "cancelled"),
            "expected cancellation error, got {err:?}"
        );
        assert_eq!(
            inner.calls.lock().unwrap().as_slice(),
            &["body", "first_nested"]
        );
    }

    struct SlowEval {
        sleep_ms: u64,
    }

    impl ExprEvaluator for SlowEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, Value)],
        ) -> Result<Value, EvalError> {
            std::thread::sleep(std::time::Duration::from_millis(self.sleep_ms));
            Ok(serde_json::json!(json!("done")))
        }
    }

    #[test]
    fn cancel_race_between_spawn_and_cancel() {
        let s = DefaultScheduler::new();
        let cancel = s.cancel_runtime();
        let mut completed = 0;
        let mut cancelled = 0;

        for i in 0..20 {
            let token = cancel.cancel_token();
            let cancel_t = cancel.clone();
            let t_token = token.clone();
            // Even iterations cancel immediately; odd iterations cancel after the
            // slow task should have finished, exercising the race window.
            let delay = if i % 2 == 0 { 0 } else { 30 };

            let mut ctx = EvalContext::new();
            ctx.set_expr_evaluator(Rc::new(SlowEval { sleep_ms: 10 }));

            let task = s.spawn_cancelable(token, work("slow"), &mut ctx).unwrap();

            let t = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(delay));
                let _ = cancel_t.cancel(t_token);
            });

            match s.await_task(task, &mut ctx) {
                Ok(_) => completed += 1,
                Err(_) => cancelled += 1,
            }

            t.join().unwrap();
        }

        // Both outcomes should have been observed at least once across the
        // retries, confirming the cancellation is racy rather than deterministic.
        assert!(completed > 0, "expected at least one completed result");
        assert!(cancelled > 0, "expected at least one cancellation");
    }

    #[test]
    fn cancel_running_task_still_returns_result_if_completed() {
        let s = DefaultScheduler::new();
        let mut ctx = EvalContext::new();
        let eval = MockEval::default();
        ctx.set_expr_evaluator(Rc::new(eval.clone()));

        let token = s.cancel_runtime().cancel_token();
        let task = s
            .spawn_cancelable(token.clone(), work("ok"), &mut ctx)
            .unwrap();

        let result = s.await_task(task.clone(), &mut ctx).unwrap();
        assert_eq!(result, serde_json::json!(json!({ "body": "ok" })));

        // Cancelling a completed task should be a no-op.
        s.cancel(token).unwrap();

        let again = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(again, result);
    }

    #[test]
    fn timeout_race_task_completes_right_before_deadline() {
        let s = DefaultScheduler::new();
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(SlowEval { sleep_ms: 10 }));

        let task = s.spawn(work("slow"), &mut ctx).unwrap();
        let result = s.timeout(task, 50, &mut ctx).unwrap();

        assert_eq!(result, serde_json::json!(json!("done")));
    }
}
