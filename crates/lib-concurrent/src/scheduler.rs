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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_deque::{Injector, Steal, Stealer, Worker};

use crate::cancel::{CancelRuntime, CancelableEval, DefaultCancelRuntime};
use crate::timer::{ThreadTimerRuntime, TimerRuntime};
use handl_vm_values_json::{Context, Error, Eval, FunctionValue, NonFunctionValue};
use serde_json::json;

/// A replaceable backend for the `concurrent.spawn`, `concurrent.await`,
/// and `concurrent.yield` primitives.
pub trait Scheduler: Send + Sync {
    /// Spawn `work` as a new task and return a `Task` value.
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error>;

    /// Wait for `task` to finish, running it via `ctx.eval_expr` if needed.
    fn await_task(
        &self,
        task: NonFunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error>;

    /// Yield control to the scheduler and run one pending task if any.
    fn yield_now(
        &self,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error>;

    /// Cancel any pending or running tasks associated with `token`.
    fn cancel(&self, token: NonFunctionValue) -> Result<(), Error> {
        let _ = token;
        Ok(())
    }

    /// Spawn `work` as a cancelable task and return a `Task` value.
    fn spawn_cancelable(
        &self,
        token: NonFunctionValue,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let _ = token;
        self.spawn(work, ctx)
    }

    /// Pause execution for `ms` milliseconds.
    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let _ = ms;
        Ok(NonFunctionValue::default())
    }

    /// Wait for `task` to finish or `ms` milliseconds to elapse.
    fn timeout(
        &self,
        task: NonFunctionValue,
        _ms: u64,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
    result: Option<NonFunctionValue>,
    error: Option<String>,
    token_id: Option<u64>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

fn task_value(id: u64, status: &str) -> NonFunctionValue {
    NonFunctionValue::new(json!({
        "type": "task",
        "id": id,
        "status": status,
    }))
}

fn task_id(task: &NonFunctionValue) -> Result<u64, Error> {
    let obj = task
        .as_object()
        .ok_or_else(|| Error::PrimitiveError("task value must be an object".to_string()))?;
    let id = obj
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::PrimitiveError("task value must contain a numeric id".to_string()))?;
    Ok(id)
}

fn cancel_token_id(token: &NonFunctionValue) -> Result<u64, Error> {
    let obj = token
        .as_object()
        .ok_or_else(|| Error::PrimitiveError("cancel token must be an object".to_string()))?;
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

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<'_, Vec<TaskRecord>>, Error> {
        self.tasks
            .lock()
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))
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
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
        token: NonFunctionValue,
        work: FunctionValue,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(Error::PrimitiveError("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

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

    fn cancel(&self, token: NonFunctionValue) -> Result<(), Error> {
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
        Ok(())
    }

    fn await_task(
        &self,
        task: NonFunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = task_id(&task)?;

        let (id, body, cancel_flag) = {
            let mut tasks = self.lock_tasks()?;
            let rec = tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| Error::PrimitiveError(format!("task {id} not found")))?;

            match rec.status {
                TaskStatus::Completed => {
                    return Ok(rec.result.clone().unwrap_or_default());
                }
                TaskStatus::Failed => {
                    return Err(Error::PrimitiveError(
                        rec.error.clone().unwrap_or_default(),
                    ));
                }
                TaskStatus::Running => {
                    return Err(Error::PrimitiveError(format!(
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
                return Err(Error::PrimitiveError("cancelled".to_string()));
            }
        }

        let res = if let Some(eval) = ctx.eval_arc() {
            let flag = cancel_flag
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
            CancelableEval::new(eval, flag).eval_expr(&body, &[])
        } else {
            if let Some(flag) = &cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        rec.status = TaskStatus::Failed;
                        rec.error = Some("cancelled".to_string());
                    }
                    return Err(Error::PrimitiveError("cancelled".to_string()));
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
                    return Err(Error::PrimitiveError("cancelled".to_string()));
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
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
                    return Err(Error::PrimitiveError("cancelled".to_string()));
                }
            }

            let res = if let Some(eval) = ctx.eval_arc() {
                let flag = cancel_flag
                    .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
                CancelableEval::new(eval, flag).eval_expr(&body, &[])
            } else {
                if let Some(flag) = &cancel_flag {
                    if flag.load(Ordering::SeqCst) {
                        let mut tasks = self.lock_tasks()?;
                        if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                            rec.status = TaskStatus::Failed;
                            rec.error = Some("cancelled".to_string());
                        }
                        return Err(Error::PrimitiveError("cancelled".to_string()));
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
                        return Err(Error::PrimitiveError("cancelled".to_string()));
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

        Ok(NonFunctionValue::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.timer.sleep(ms)?;
        Ok(NonFunctionValue::default())
    }

    fn timeout(
        &self,
        task: NonFunctionValue,
        ms: u64,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            let rec = tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| Error::PrimitiveError(format!("task {id} not found")))?;

            if let Some(token_id) = rec.token_id {
                NonFunctionValue::new(json!({
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
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

        {
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.cancel_flag = Some(cancel_flag.clone());
            }
        }

        let flag = cancel_flag;
        self.timer.schedule_boxed(ms, Box::new(move || {
            flag.store(true, Ordering::SeqCst);
        }));

        self.await_task(task, ctx)
    }
}

/// A unit of work sent to a worker thread.
enum Job {
    Execute {
        id: u64,
        body: String,
        eval: Arc<dyn Eval<NonFunctionValue> + Send + Sync>,
        cancel_flag: Option<Arc<AtomicBool>>,
    },
    Shutdown,
}

type EvalResult = Result<NonFunctionValue, Error>;

fn worker_thread(jobs: Arc<Mutex<mpsc::Receiver<Job>>>, results: mpsc::Sender<(u64, EvalResult)>) {
    while let Ok(job) = jobs.lock().unwrap().recv() {
        match job {
            Job::Shutdown => break,
            Job::Execute { id, body, eval, cancel_flag } => {
                let flag = cancel_flag
                    .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
                let eval = CancelableEval::new(eval, flag);

                let res = eval.eval_expr(&body, &[]);

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

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<'_, Vec<TaskRecord>>, Error> {
        self.tasks
            .lock()
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))
    }

    fn lock_results(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, mpsc::Receiver<(u64, EvalResult)>>, Error> {
        self.results_rx
            .lock()
            .map_err(|_| Error::PrimitiveError("result channel lock poisoned".to_string()))
    }

    fn process_results(&self) -> Result<(), Error> {
        let results = self.lock_results()?;
        loop {
            match results.try_recv() {
                Ok((id, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                        if rec.status == TaskStatus::Pending
                            || rec.status == TaskStatus::Running
                        {
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
                Err(mpsc::TryRecvError::Empty) => break Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => break Ok(()),
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
    pub fn shutdown(&self) -> Result<Vec<bool>, Error> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(Vec::new());
        }

        let worker_count = {
            let guard = self
                .worker_handles
                .lock()
                .map_err(|_| Error::PrimitiveError("worker handles lock poisoned".to_string()))?;
            guard.as_ref().map(|v| v.len()).unwrap_or(0)
        };

        if worker_count == 0 {
            return Ok(Vec::new());
        }

        // Queue a shutdown job for each worker and close the job sender.
        let sender = self
            .jobs_tx
            .lock()
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))?
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
                .map_err(|_| Error::PrimitiveError("worker handles lock poisoned".to_string()))?;
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

        Ok(joined)
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
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let eval = ctx
            .eval_arc()
            .ok_or_else(|| Error::PrimitiveError(
                "ThreadPoolScheduler requires a Send + Sync eval callback; \
                 set one with CanonicalContext::set_sync_eval".to_string(),
            ))?;

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
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))?;
        let tx = guard
            .as_ref()
            .ok_or_else(|| Error::PrimitiveError("scheduler shut down".to_string()))?;
        if tx
            .send(Job::Execute { id, body, eval, cancel_flag: None })
            .is_err()
        {
            drop(guard);
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.status = TaskStatus::Failed;
                rec.error = Some("scheduler shut down".to_string());
            }
            return Err(Error::PrimitiveError("scheduler shut down".to_string()));
        }

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn spawn_cancelable(
        &self,
        token: NonFunctionValue,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let eval = ctx
            .eval_arc()
            .ok_or_else(|| Error::PrimitiveError(
                "ThreadPoolScheduler requires a Send + Sync eval callback; \
                 set one with CanonicalContext::set_sync_eval".to_string(),
            ))?;

        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(Error::PrimitiveError("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

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
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))?;
        let tx = guard
            .as_ref()
            .ok_or_else(|| Error::PrimitiveError("scheduler shut down".to_string()))?;
        if tx
            .send(Job::Execute { id, body, eval, cancel_flag: Some(cancel_flag) })
            .is_err()
        {
            drop(guard);
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                rec.status = TaskStatus::Failed;
                rec.error = Some("scheduler shut down".to_string());
            }
            return Err(Error::PrimitiveError("scheduler shut down".to_string()));
        }

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn cancel(&self, token: NonFunctionValue) -> Result<(), Error> {
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
        Ok(())
    }

    fn await_task(
        &self,
        task: NonFunctionValue,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
                            return Err(Error::PrimitiveError(
                                rec.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            rec.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(Error::PrimitiveError(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            // Wait for a result, then re-process.
            let (rid, res) = self
                .lock_results()?
                .recv()
                .map_err(|_| Error::PrimitiveError("result channel closed".to_string()))?;

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
    }

    fn yield_now(
        &self,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.process_results()?;
        Ok(NonFunctionValue::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.timer.sleep(ms)?;
        Ok(NonFunctionValue::default())
    }

    fn timeout(
        &self,
        task: NonFunctionValue,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            if let Some(rec) = tasks.iter_mut().find(|t| t.id == id) {
                if let Some(token_id) = rec.token_id {
                    NonFunctionValue::new(json!({
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
                return Err(Error::PrimitiveError(format!("task {id} not found")));
            }
        };

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

        let (timeout_tx, timeout_rx) = mpsc::channel();
        let flag = cancel_flag;
        self.timer.schedule_boxed(ms, Box::new(move || {
            flag.store(true, Ordering::SeqCst);
            let _ = timeout_tx.send(());
        }));

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
                            return Err(Error::PrimitiveError(
                                rec.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            rec.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(Error::PrimitiveError(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            if timeout_rx.try_recv().is_ok() {
                let _ = self.cancel(token.clone())?;
                continue;
            }

            match self
                .lock_results()?
                .recv_timeout(Duration::from_millis(1))
            {
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
                    return Err(Error::PrimitiveError("result channel closed".to_string()));
                }
            }
        }
    }

    fn shutdown(&self) {
        let _ = ThreadPoolScheduler::shutdown(self);
    }
}

/// Result of executing a `WorkStealingJob`.
enum TaskResult {
    Ok(NonFunctionValue),
    Err(String),
    Cancelled,
}

/// Per-task state for the work-stealing scheduler.
struct TaskState {
    status: TaskStatus,
    result: Option<NonFunctionValue>,
    error: Option<String>,
    token_id: Option<u64>,
}

/// A unit of work for the work-stealing thread pool.
enum WsJob {
    Execute {
        id: u64,
        body: String,
        eval: Arc<dyn Eval<NonFunctionValue> + Send + Sync>,
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
                eval,
                cancel_flag,
                result_tx,
            }) => {
                let eval = CancelableEval::new(eval, cancel_flag);
                match eval.eval_expr(&body, &[]) {
                    Ok(value) => {
                        let _ = result_tx.send((id, TaskResult::Ok(value)));
                    }
                    Err(Error::PrimitiveError(ref m)) if m == "cancelled" => {
                        let _ = result_tx.send((id, TaskResult::Cancelled));
                    }
                    Err(e) => {
                        let _ = result_tx.send((id, TaskResult::Err(e.to_string())));
                    }
                }
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

        let worker_queue: Vec<Worker<WsJob>> =
            (0..workers).map(|_| Worker::new_fifo()).collect();
        let all_stealers: Vec<Stealer<WsJob>> =
            worker_queue.iter().map(|w| w.stealer()).collect();

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

    fn lock_tasks(&self) -> Result<std::sync::MutexGuard<'_, HashMap<u64, TaskState>>, Error> {
        self.tasks
            .lock()
            .map_err(|_| Error::PrimitiveError("scheduler lock poisoned".to_string()))
    }

    fn lock_results(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, mpsc::Receiver<(u64, TaskResult)>>, Error> {
        self.results_rx
            .lock()
            .map_err(|_| Error::PrimitiveError("result channel lock poisoned".to_string()))
    }

    fn process_results(&self) -> Result<(), Error> {
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
                Err(mpsc::TryRecvError::Empty) => break Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => break Ok(()),
            }
        }
    }

    /// Gracefully shut down the work-stealing pool.
    pub fn shutdown(&self) -> Result<Vec<bool>, Error> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(Vec::new());
        }

        let worker_count = {
            let guard = self
                .worker_handles
                .lock()
                .map_err(|_| Error::PrimitiveError("worker handles lock poisoned".to_string()))?;
            guard.as_ref().map(|v| v.len()).unwrap_or(0)
        };

        if worker_count == 0 {
            return Ok(Vec::new());
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
                .map_err(|_| Error::PrimitiveError("worker handles lock poisoned".to_string()))?;
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

        Ok(joined)
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
    fn spawn(
        &self,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let eval = ctx
            .eval_arc()
            .ok_or_else(|| {
                Error::PrimitiveError(
                    "WorkStealingScheduler requires a Send + Sync eval callback; \
                     set one with CanonicalContext::set_sync_eval"
                        .to_string(),
                )
            })?;

        if self.shutdown.load(Ordering::SeqCst) {
            return Err(Error::PrimitiveError("scheduler shut down".to_string()));
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
            eval,
            cancel_flag,
            result_tx: self.results_tx.clone(),
        });

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn spawn_cancelable(
        &self,
        token: NonFunctionValue,
        work: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let eval = ctx
            .eval_arc()
            .ok_or_else(|| {
                Error::PrimitiveError(
                    "WorkStealingScheduler requires a Send + Sync eval callback; \
                     set one with CanonicalContext::set_sync_eval"
                        .to_string(),
                )
            })?;

        let token_id = cancel_token_id(&token)?;

        if self.cancel_runtime.is_cancelled(&token) {
            return Err(Error::PrimitiveError("cancelled".to_string()));
        }

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

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
            eval,
            cancel_flag,
            result_tx: self.results_tx.clone(),
        });

        Ok(task_value(id, TaskStatus::Pending.as_str()))
    }

    fn cancel(&self, token: NonFunctionValue) -> Result<(), Error> {
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
        Ok(())
    }

    fn await_task(
        &self,
        task: NonFunctionValue,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
                            return Err(Error::PrimitiveError(
                                state.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            state.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(Error::PrimitiveError(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            let (rid, res) = self
                .lock_results()?
                .recv()
                .map_err(|_| Error::PrimitiveError("result channel closed".to_string()))?;

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
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.process_results()?;

        let job = match self.injector.steal() {
            Steal::Success(j) => j,
            _ => return Ok(NonFunctionValue::default()),
        };

        match job {
            WsJob::Shutdown => {
                self.injector.push(WsJob::Shutdown);
            }
            WsJob::Execute {
                id,
                body,
                eval,
                cancel_flag,
                result_tx,
            } => {
                let eval = CancelableEval::new(eval, cancel_flag);
                match eval.eval_expr(&body, &[]) {
                    Ok(value) => {
                        let _ = result_tx.send((id, TaskResult::Ok(value)));
                    }
                    Err(Error::PrimitiveError(ref m)) if m == "cancelled" => {
                        let _ = result_tx.send((id, TaskResult::Cancelled));
                    }
                    Err(e) => {
                        let _ = result_tx.send((id, TaskResult::Err(e.to_string())));
                    }
                }
                self.process_results()?;
            }
        }

        Ok(NonFunctionValue::default())
    }

    fn sleep(
        &self,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.timer.sleep(ms)?;
        Ok(NonFunctionValue::default())
    }

    fn timeout(
        &self,
        task: NonFunctionValue,
        ms: u64,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = task_id(&task)?;

        let token = {
            let mut tasks = self.lock_tasks()?;
            if let Some(state) = tasks.get_mut(&id) {
                if let Some(token_id) = state.token_id {
                    NonFunctionValue::new(json!({
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
                return Err(Error::PrimitiveError(format!("task {id} not found")));
            }
        };

        let cancel_flag = self
            .cancel_runtime
            .cancel_flag(&token)
            .ok_or_else(|| Error::PrimitiveError("cancel token not found".to_string()))?;

        let (timeout_tx, timeout_rx) = mpsc::channel();
        let flag = cancel_flag;
        self.timer.schedule_boxed(ms, Box::new(move || {
            flag.store(true, Ordering::SeqCst);
            let _ = timeout_tx.send(());
        }));

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
                            return Err(Error::PrimitiveError(
                                state.error.clone().unwrap_or_default(),
                            ));
                        }
                        TaskStatus::Running | TaskStatus::Pending => {
                            state.status = TaskStatus::Running;
                            true
                        }
                    }
                } else {
                    return Err(Error::PrimitiveError(format!("task {id} not found")));
                }
            };

            if !running {
                continue;
            }

            if timeout_rx.try_recv().is_ok() {
                let _ = self.cancel(token.clone())?;
                continue;
            }

            match self
                .lock_results()?
                .recv_timeout(Duration::from_millis(1))
            {
                Ok((rid, res)) => {
                    let mut tasks = self.lock_tasks()?;
                    if let Some(state) = tasks.get_mut(&rid) {
                        if state.status == TaskStatus::Pending || state.status == TaskStatus::Running
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
                    return Err(Error::PrimitiveError("result channel closed".to_string()));
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
    use handl_vm_values_json::{CanonicalContext, Eval};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockEval {
        calls: Arc<Mutex<Vec<String>>>,
        cancel: Option<Arc<AtomicBool>>,
    }

    impl Eval<NonFunctionValue> for MockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            if let Some(flag) = &self.cancel {
                if flag.load(Ordering::SeqCst) {
                    return Err(Error::PrimitiveError("cancelled".to_string()));
                }
            }
            self.calls.lock().unwrap().push(expr.to_string());
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

    #[test]
    fn spawn_returns_task_value() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn await_runs_task_body() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();
        let result = s.await_task(task, &mut ctx).unwrap();

        assert_eq!(result, NonFunctionValue::new(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);
    }

    #[test]
    fn yield_now_runs_a_pending_task() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

        let s = DefaultScheduler::new();
        let task = s.spawn(work("work()"), &mut ctx).unwrap();

        let _ = s.yield_now(&mut ctx).unwrap();
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["work()"]);

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "work()" })));
        assert_eq!(eval.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn thread_pool_scheduler_spawns_and_awaits() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);
    }

    #[test]
    fn default_cancel_pending_prevents_execution() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

        let s = DefaultScheduler::new();
        let token = s.cancel_runtime().cancel_token();
        let task = s.spawn_cancelable(token.clone(), work("hello()"), &mut ctx).unwrap();

        s.cancel(token).unwrap();

        let err = s.await_task(task, &mut ctx);
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn default_cancelable_already_cancelled() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_eval(Box::new(eval.clone()));

        let s = DefaultScheduler::new();
        let token = s.cancel_runtime().cancel_token();
        s.cancel_runtime().cancel(token.clone()).unwrap();

        let err = s.spawn_cancelable(token, work("hello()"), &mut ctx);
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn thread_pool_cancel_pending_prevents_execution() {
        let s = ThreadPoolScheduler::new(1);
        let token = s.cancel_runtime().cancel_token();
        let flag = s.cancel_runtime().cancel_flag(&token).unwrap();

        let mut ctx = CanonicalContext::new();
        let eval = MockEval {
            cancel: Some(flag),
            ..Default::default()
        };
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let task = s.spawn_cancelable(token.clone(), work("hello()"), &mut ctx).unwrap();

        s.cancel(token).unwrap();

        let err = s.await_task(task, &mut ctx);
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn thread_pool_cancelable_already_cancelled() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let token = s.cancel_runtime().cancel_token();
        s.cancel_runtime().cancel(token.clone()).unwrap();

        let err = s.spawn_cancelable(token, work("hello()"), &mut ctx);
        assert!(matches!(err, Err(Error::PrimitiveError(_))));
        assert!(eval.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn thread_pool_shutdown_joins_workers() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();
        let result = s.await_task(task, &mut ctx).unwrap();

        assert_eq!(result, NonFunctionValue::new(json!({ "body": "hello()" })));
        assert_eq!(eval.calls.lock().unwrap().as_slice(), &["hello()"]);

        let joined = s.shutdown().unwrap();
        assert_eq!(joined.len(), 2);
        assert!(joined.iter().all(|&j| j), "all workers should join");
    }

    #[test]
    fn thread_pool_shutdown_cancels_pending() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = ThreadPoolScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        // Shut down immediately. The task may or may not have started.
        let joined = s.shutdown().unwrap();
        assert_eq!(joined.len(), 2);

        let err = s.await_task(task, &mut ctx).unwrap_err();
        assert!(
            matches!(err, Error::PrimitiveError(ref msg) if msg == "scheduler shut down"),
            "expected shutdown error, got {err:?}"
        );
    }

    #[test]
    fn thread_pool_scheduler_drops_cleanly() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        {
            let s = ThreadPoolScheduler::new(2);
            let _ = s.spawn(work("hello()"), &mut ctx);
            // Dropping `s` should shut down the pool without panicking.
        }

        // Reaching this point means the scheduler dropped without hanging.
        let _ = eval;
    }

    #[test]
    fn work_stealing_spawn_and_await() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = WorkStealingScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        assert_eq!(task.get("type"), Some(&json!("task")));
        assert_eq!(task.get("id"), Some(&json!(0)));
        assert_eq!(task.get("status"), Some(&json!("pending")));

        let result = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "hello()" })));
        assert!(eval.calls.lock().unwrap().contains(&"hello()".to_string()));

        let _ = s.shutdown();
    }

    #[test]
    fn work_stealing_spawn_many() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

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
            assert_eq!(result, NonFunctionValue::new(json!({ "body": bodies[i] })));
        }

        assert_eq!(eval.calls.lock().unwrap().len(), 100);
        let _ = s.shutdown();
    }

    #[test]
    fn work_stealing_cancel_pending() {
        let s = WorkStealingScheduler::new(2);
        let token = s.cancel_runtime().cancel_token();

        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let task1 = s.spawn_cancelable(token.clone(), work("one()"), &mut ctx).unwrap();
        let task2 = s.spawn_cancelable(token.clone(), work("two()"), &mut ctx).unwrap();

        s.cancel(token).unwrap();

        assert!(s.await_task(task1, &mut ctx).is_err());
        assert!(s.await_task(task2, &mut ctx).is_err());

        let _ = s.shutdown();
    }

    #[test]
    fn work_stealing_shutdown() {
        let mut ctx = CanonicalContext::new();
        let eval = MockEval::default();
        ctx.set_sync_eval(Arc::new(eval.clone()));

        let s = WorkStealingScheduler::new(2);
        let task = s.spawn(work("hello()"), &mut ctx).unwrap();

        let joined = s.shutdown().unwrap();
        assert_eq!(joined.len(), 2);

        let err = s.await_task(task, &mut ctx).unwrap_err();
        assert!(
            matches!(err, Error::PrimitiveError(ref msg) if msg == "scheduler shut down"),
            "expected shutdown error, got {err:?}"
        );
    }

    struct NestedMockEval {
        calls: Arc<Mutex<Vec<String>>>,
        flag: Arc<AtomicBool>,
        wrapped: Arc<Mutex<Option<Arc<CancelableEval>>>>,
    }

    impl NestedMockEval {
        fn new(flag: Arc<AtomicBool>) -> Arc<Self> {
            Arc::new(Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                flag,
                wrapped: Arc::new(Mutex::new(None)),
            })
        }

        fn set_wrapped(&self, wrapped: Arc<CancelableEval>) {
            *self.wrapped.lock().unwrap() = Some(wrapped);
        }
    }

    impl Eval<NonFunctionValue> for NestedMockEval {
        fn eval_expr(
            &self,
            expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
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
        let wrapped = Arc::new(CancelableEval::new(inner.clone(), flag.clone()));
        inner.set_wrapped(wrapped.clone());

        let mut ctx = CanonicalContext::new();
        ctx.set_sync_eval(wrapped);

        let task = s.spawn_cancelable(token, work("body"), &mut ctx).unwrap();
        let err = s.await_task(task, &mut ctx).unwrap_err();

        assert!(
            matches!(err, Error::PrimitiveError(ref m) if m == "cancelled"),
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

    impl Eval<NonFunctionValue> for SlowEval {
        fn eval_expr(
            &self,
            _expr: &str,
            _vars: &[(String, NonFunctionValue)],
        ) -> Result<NonFunctionValue, Error> {
            std::thread::sleep(std::time::Duration::from_millis(self.sleep_ms));
            Ok(NonFunctionValue::new(json!("done")))
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

            let mut ctx = CanonicalContext::new();
            ctx.set_eval(Box::new(SlowEval { sleep_ms: 15 }));

            let task = s
                .spawn_cancelable(token, work("slow"), &mut ctx)
                .unwrap();

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
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(MockEval::default()));

        let token = s.cancel_runtime().cancel_token();
        let task = s
            .spawn_cancelable(token.clone(), work("ok"), &mut ctx)
            .unwrap();

        let result = s.await_task(task.clone(), &mut ctx).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "ok" })));

        // Cancelling a completed task should be a no-op.
        s.cancel(token).unwrap();

        let again = s.await_task(task, &mut ctx).unwrap();
        assert_eq!(again, result);
    }

    #[test]
    fn timeout_race_task_completes_right_before_deadline() {
        let s = DefaultScheduler::new();
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(SlowEval { sleep_ms: 30 }));

        let task = s.spawn(work("slow"), &mut ctx).unwrap();
        let result = s.timeout(task, 50, &mut ctx).unwrap();

        assert_eq!(result, NonFunctionValue::new(json!("done")));
    }
}
