//! Synchronization primitives backend for `handl-lib-concurrent`.
//!
//! The [`SyncRuntime`] trait is a **replaceable backend** for the
//! synchronization primitives exposed by [`ConcurrencyModule`].
//! [`DefaultSyncRuntime`] is the bundled, in-memory, `std`-based default.
//!
//! A host builder can swap the sync backend by providing any
//! `Arc<dyn SyncRuntime + Send + Sync>`.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::function_value::FunctionValue;
use handl_platform::{EvalContext, EvalError};
use serde_json::Value;

/// A replaceable backend for the synchronization primitives:
/// `channel`, `send`, `recv`, `mutex`, `lock`, `unlock`, `rwlock`,
/// `read_lock`, `write_lock`, `semaphore`, `acquire`, and `release`.
pub trait SyncRuntime {
    /// Creates a new channel, optionally bounded by `capacity`.
    fn channel(&self, capacity: Option<Value>) -> Value;

    /// Sends `value` on `channel`.
    fn send(&self, channel: Value, value: Value) -> Result<Value, EvalError>;

    /// Tries to send `value` on `channel` without blocking.
    fn try_send(&self, channel: Value, value: Value) -> Result<Value, EvalError>;

    /// Receives a value from `channel`.
    fn recv(&self, channel: Value) -> Result<Value, EvalError>;

    /// Tries to receive a value from `channel` without blocking.
    fn try_recv(&self, channel: &Value) -> Result<Value, EvalError> {
        self.recv(channel.clone())
    }

    /// Polls `channels` and returns `[index, value]` for the first channel
    /// that has a value available, or `WouldBlock` if all are empty.
    fn select(&self, channels: &[Value]) -> Result<Value, EvalError>;

    /// Creates a new mutex.
    fn mutex(&self) -> Value;

    /// Acquires `mutex`, runs `thunk`, and releases the lock.
    fn lock(
        &self,
        mutex: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Tries to acquire `mutex` and, if available, runs `thunk` without blocking.
    fn try_lock(
        &self,
        mutex: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Releases `mutex`.
    fn unlock(&self, mutex: Value) -> Result<Value, EvalError>;

    /// Creates a new read-write lock.
    fn rwlock(&self) -> Value;

    /// Acquires a read lock on `rwlock` and runs `thunk`.
    fn read_lock(
        &self,
        rwlock: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Acquires a write lock on `rwlock` and runs `thunk`.
    fn write_lock(
        &self,
        rwlock: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError>;

    /// Creates a new semaphore with `permits` initial permits.
    fn semaphore(&self, permits: Value) -> Value;

    /// Acquires `n` permits from `semaphore`.
    fn acquire(&self, semaphore: Value, n: Value) -> Result<Value, EvalError>;

    /// Tries to acquire `n` permits from `semaphore` without blocking.
    fn try_acquire(&self, semaphore: Value, n: Value) -> Result<Value, EvalError>;

    /// Releases `n` permits to `semaphore`.
    fn release(&self, semaphore: Value, n: Value) -> Result<Value, EvalError>;
}

/// Creates a resource value of the form `{"type": <kind>, "id": <id>}`.
pub fn resource_value(kind: &str, id: u64) -> Value {
    serde_json::json!({"type": kind, "id": id})
}

/// Parses a resource value and returns its `id` after checking its `type`.
pub fn resource_id(value: &Value, expected_kind: &str) -> Result<u64, EvalError> {
    let obj = value
        .as_object()
        .ok_or_else(|| EvalError::Other("expected a resource object".to_string()))?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EvalError::Other("resource missing 'type'".to_string()))?;
    if kind != expected_kind {
        return Err(EvalError::Other(format!(
            "expected resource type {expected_kind}, got {kind}"
        )));
    }
    obj.get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| EvalError::Other("resource missing numeric 'id'".to_string()))
}

/// Parses a `Value` as an unsigned integer.
pub fn as_u64(value: &Value) -> Result<u64, EvalError> {
    value
        .as_u64()
        .ok_or_else(|| EvalError::Other("expected a non-negative integer".to_string()))
}

fn poison<T>(_: std::sync::PoisonError<T>) -> SyncError {
    SyncError::Poisoned
}

/// Errors raised by a [`SyncRuntime`] backend.
///
/// These are intentionally distinct from VM errors so that the host
/// functions in [`ConcurrencyModule`](crate::ConcurrencyModule) can
/// translate a `WouldBlock` condition into an effect-style
/// [`handl_plugin_vm::EvalError::Operation`] for the scheduler to intercept.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// The operation would block and should be retried/parked by the scheduler.
    WouldBlock,
    /// A mutex/guard protecting the runtime state was poisoned.
    Poisoned,
    /// The requested resource (channel, mutex, etc.) was not found.
    NotFound(String),
    /// The resource value or argument was malformed.
    BadResource(String),
    /// A generic VM error propagated from running a thunk.
    Other(EvalError),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::WouldBlock => write!(f, "would block"),
            SyncError::Poisoned => write!(f, "sync runtime lock poisoned"),
            SyncError::NotFound(msg) => write!(f, "{msg}"),
            SyncError::BadResource(msg) => write!(f, "{msg}"),
            SyncError::Other(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<EvalError> for SyncError {
    fn from(err: EvalError) -> Self {
        SyncError::Other(err)
    }
}

impl From<SyncError> for EvalError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Other(e) => e,
            other => EvalError::Other(other.to_string()),
        }
    }
}

#[derive(Debug)]
struct Channel {
    capacity: Option<usize>,
    queue: VecDeque<Value>,
    closed: bool,
}

/// An in-memory, `std`-based synchronization runtime.
#[derive(Debug)]
pub struct DefaultSyncRuntime {
    next_id: AtomicU64,
    channels: Mutex<HashMap<u64, Arc<Mutex<Channel>>>>,
    mutexes: Mutex<HashMap<u64, Arc<Mutex<bool>>>>,
    rwlocks: Mutex<HashMap<u64, Arc<RwLock<()>>>>,
    semaphores: Mutex<HashMap<u64, Arc<Mutex<u64>>>>,
}

impl DefaultSyncRuntime {
    /// Creates a new empty sync runtime.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            channels: Mutex::new(HashMap::new()),
            mutexes: Mutex::new(HashMap::new()),
            rwlocks: Mutex::new(HashMap::new()),
            semaphores: Mutex::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Marks a channel as closed. Existing queued values may still be received.
    pub fn close(&self, channel: Value) -> Result<Value, EvalError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        chan.closed = true;
        Ok(Value::default())
    }
}

impl Default for DefaultSyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncRuntime for DefaultSyncRuntime {
    fn channel(&self, capacity: Option<Value>) -> Value {
        let cap = capacity.and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_u64().map(|c| c as usize)
            }
        });
        let id = self.next_id();
        let mut channels = self.channels.lock().unwrap();
        channels.insert(
            id,
            Arc::new(Mutex::new(Channel {
                capacity: cap,
                queue: VecDeque::new(),
                closed: false,
            })),
        );
        resource_value("channel", id)
    }

    fn send(&self, channel: Value, value: Value) -> Result<Value, EvalError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        if chan.closed {
            return Err(EvalError::from(SyncError::NotFound(format!(
                "channel {id} is closed"
            ))));
        }
        if let Some(cap) = chan.capacity {
            if chan.queue.len() >= cap {
                return Err(EvalError::from(SyncError::WouldBlock));
            }
        }
        chan.queue.push_back(value);
        Ok(Value::default())
    }

    fn try_send(&self, channel: Value, value: Value) -> Result<Value, EvalError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        if chan.closed {
            return Err(EvalError::from(SyncError::NotFound(format!(
                "channel {id} is closed"
            ))));
        }
        if let Some(cap) = chan.capacity {
            if chan.queue.len() >= cap {
                return Err(EvalError::from(SyncError::WouldBlock));
            }
        }
        chan.queue.push_back(value);
        Ok(Value::default())
    }

    fn recv(&self, channel: Value) -> Result<Value, EvalError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        if let Some(value) = chan.queue.pop_front() {
            return Ok(value);
        }
        if chan.closed {
            Err(EvalError::from(SyncError::NotFound(format!(
                "channel {id} is closed"
            ))))
        } else {
            Err(EvalError::from(SyncError::WouldBlock))
        }
    }

    fn select(&self, channels: &[Value]) -> Result<Value, EvalError> {
        for (i, channel) in channels.iter().enumerate() {
            if let Ok(value) = self.try_recv(channel) {
                return Ok(serde_json::json!([i, value.clone()]));
            }
        }
        Err(EvalError::from(SyncError::WouldBlock))
    }

    fn mutex(&self) -> Value {
        let id = self.next_id();
        let mut mutexes = self.mutexes.lock().unwrap();
        mutexes.insert(id, Arc::new(Mutex::new(false)));
        resource_value("mutex", id)
    }

    fn lock(
        &self,
        mutex: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.lock().map_err(poison)?;
        if *guard {
            return Err(EvalError::from(SyncError::BadResource(
                "mutex already locked".to_string(),
            )));
        }
        *guard = true;
        let result = ctx.eval_expr(&thunk.body, &[])?;
        *guard = false;
        drop(guard);
        Ok(result)
    }

    fn try_lock(
        &self,
        mutex: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.try_lock().map_err(|_| SyncError::WouldBlock)?;
        if *guard {
            return Err(EvalError::from(SyncError::WouldBlock));
        }
        *guard = true;
        let result = ctx.eval_expr(&thunk.body, &[])?;
        *guard = false;
        Ok(result)
    }

    fn unlock(&self, mutex: Value) -> Result<Value, EvalError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.lock().map_err(poison)?;
        if !*guard {
            return Err(EvalError::from(SyncError::BadResource(
                "mutex not locked".to_string(),
            )));
        }
        *guard = false;
        Ok(Value::default())
    }

    fn rwlock(&self) -> Value {
        let id = self.next_id();
        let mut rwlocks = self.rwlocks.lock().unwrap();
        rwlocks.insert(id, Arc::new(RwLock::new(())));
        resource_value("rwlock", id)
    }

    fn read_lock(
        &self,
        rwlock: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&rwlock, "rwlock")?;
        let rwlocks = self.rwlocks.lock().map_err(poison)?;
        let rw = rwlocks
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("rwlock {id} not found")))?
            .clone();
        drop(rwlocks);
        let _guard = rw.read().map_err(poison)?;
        Ok(ctx.eval_expr(&thunk.body, &[])?)
    }

    fn write_lock(
        &self,
        rwlock: Value,
        thunk: FunctionValue,
        ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&rwlock, "rwlock")?;
        let rwlocks = self.rwlocks.lock().map_err(poison)?;
        let rw = rwlocks
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("rwlock {id} not found")))?
            .clone();
        drop(rwlocks);
        let _guard = rw.write().map_err(poison)?;
        Ok(ctx.eval_expr(&thunk.body, &[])?)
    }

    fn semaphore(&self, permits: Value) -> Value {
        let id = self.next_id();
        let count = as_u64(&permits).unwrap_or(0);
        let mut semaphores = self.semaphores.lock().unwrap();
        semaphores.insert(id, Arc::new(Mutex::new(count)));
        resource_value("semaphore", id)
    }

    fn acquire(&self, semaphore: Value, n: Value) -> Result<Value, EvalError> {
        let id = resource_id(&semaphore, "semaphore")?;
        let n = as_u64(&n)?;
        let semaphores = self.semaphores.lock().map_err(poison)?;
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);
        let mut permits = s.lock().map_err(poison)?;
        if *permits < n {
            return Err(EvalError::from(SyncError::WouldBlock));
        }
        *permits -= n;
        Ok(Value::default())
    }

    fn try_acquire(&self, semaphore: Value, n: Value) -> Result<Value, EvalError> {
        let id = resource_id(&semaphore, "semaphore")?;
        let n = as_u64(&n)?;
        let semaphores = self.semaphores.lock().map_err(poison)?;
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);
        let mut permits = s.lock().map_err(poison)?;
        if *permits < n {
            return Err(EvalError::from(SyncError::WouldBlock));
        }
        *permits -= n;
        Ok(Value::default())
    }

    fn release(&self, semaphore: Value, n: Value) -> Result<Value, EvalError> {
        let id = resource_id(&semaphore, "semaphore")?;
        let n = as_u64(&n)?;
        let semaphores = self.semaphores.lock().map_err(poison)?;
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);
        let mut permits = s.lock().map_err(poison)?;
        *permits += n;
        Ok(Value::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::ExprEvaluator;
    // `json` macro is currently unused in these tests but retained for future
    // test cases; see ADR 0040 migration notes.
    #[allow(unused_imports)]
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
            Ok(serde_json::json!({ "body": expr }))
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
    fn channel_send_recv() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(None);

        let value = serde_json::json!(1);
        assert!(s.send(ch.clone(), value).is_ok());

        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(1));
        assert_eq!(s.recv(ch), Err(EvalError::from(SyncError::WouldBlock)));
    }

    #[test]
    fn bounded_channel_send_full_blocks() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(2)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert!(s.send(ch.clone(), serde_json::json!(2)).is_ok());
        assert_eq!(
            s.send(ch.clone(), serde_json::json!(3)),
            Err(EvalError::from(SyncError::WouldBlock))
        );
    }

    #[test]
    fn channel_recv_empty_blocks() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(None);
        assert_eq!(s.recv(ch), Err(EvalError::from(SyncError::WouldBlock)));
    }

    #[test]
    fn bounded_channel_round_trip() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(2)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert!(s.send(ch.clone(), serde_json::json!(2)).is_ok());
        assert_eq!(
            s.send(ch.clone(), serde_json::json!(3)),
            Err(EvalError::from(SyncError::WouldBlock))
        );

        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(1));
        assert!(s.send(ch.clone(), serde_json::json!(3)).is_ok());
        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(2));
        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(3));
        assert_eq!(s.recv(ch), Err(EvalError::from(SyncError::WouldBlock)));
    }

    #[test]
    fn mutex_lock_unlock() {
        let s = DefaultSyncRuntime::new();
        let m = s.mutex();

        let result = s.lock(m.clone(), thunk("locked!"), &mut ctx()).unwrap();
        assert_eq!(result, serde_json::json!({ "body": "locked!" }));

        // lock auto-releases, so an explicit unlock on an unlocked mutex errors
        assert!(s.unlock(m).is_err());
    }

    #[test]
    fn semaphore_acquire_release() {
        let s = DefaultSyncRuntime::new();
        let sem = s.semaphore(serde_json::json!(3));

        assert!(s.acquire(sem.clone(), serde_json::json!(2)).is_ok());
        assert!(s.acquire(sem.clone(), serde_json::json!(2)).is_err());
        assert!(s.release(sem.clone(), serde_json::json!(2)).is_ok());
        assert!(s.acquire(sem, serde_json::json!(2)).is_ok());
    }

    #[test]
    fn rwlock_read_write() {
        let s = DefaultSyncRuntime::new();
        let rw = s.rwlock();

        assert!(s.read_lock(rw.clone(), thunk("read"), &mut ctx()).is_ok());
        assert!(s.write_lock(rw, thunk("write"), &mut ctx()).is_ok());
    }

    #[test]
    fn try_send_full_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(2)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert!(s.send(ch.clone(), serde_json::json!(2)).is_ok());
        assert_eq!(
            s.try_send(ch.clone(), serde_json::json!(3)),
            Err(EvalError::from(SyncError::WouldBlock))
        );
        assert_eq!(s.recv(ch).unwrap(), serde_json::json!(1));
    }

    #[test]
    fn try_acquire_insufficient_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let sem = s.semaphore(serde_json::json!(2));

        assert!(s.try_acquire(sem.clone(), serde_json::json!(1)).is_ok());
        assert!(s.try_acquire(sem.clone(), serde_json::json!(1)).is_ok());
        assert_eq!(
            s.try_acquire(sem, serde_json::json!(1)),
            Err(EvalError::from(SyncError::WouldBlock))
        );
    }

    #[test]
    fn try_lock_contended_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let m = s.mutex();
        let id = resource_id(&m, "mutex").unwrap();
        let inner = s.mutexes.lock().unwrap().get(&id).unwrap().clone();

        {
            let mut guard = inner.lock().unwrap();
            *guard = true;
            assert_eq!(
                s.try_lock(m.clone(), thunk("work"), &mut ctx()),
                Err(EvalError::from(SyncError::WouldBlock))
            );
            *guard = false;
        }

        assert_eq!(
            s.try_lock(m, thunk("ok"), &mut ctx()).unwrap(),
            serde_json::json!({ "body": "ok" })
        );
    }

    #[test]
    fn bounded_channel_blocks_on_second_send() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(1)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert_eq!(
            s.send(ch.clone(), serde_json::json!(2)),
            Err(EvalError::from(SyncError::WouldBlock))
        );
        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(1));
        assert!(s.send(ch.clone(), serde_json::json!(2)).is_ok());
        assert_eq!(s.recv(ch).unwrap(), serde_json::json!(2));
    }

    #[test]
    fn bounded_channel_try_send_full_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(1)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert_eq!(
            s.try_send(ch, serde_json::json!(2)),
            Err(EvalError::from(SyncError::WouldBlock))
        );
    }

    #[test]
    fn bounded_channel_close_then_send_error() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(2)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert!(s.close(ch.clone()).is_ok());
        assert!(s.send(ch, serde_json::json!(2)).is_err());
    }

    #[test]
    fn bounded_channel_close_then_recv_drains_then_error() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(serde_json::json!(2)));

        assert!(s.send(ch.clone(), serde_json::json!(1)).is_ok());
        assert!(s.send(ch.clone(), serde_json::json!(2)).is_ok());
        assert!(s.close(ch.clone()).is_ok());
        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(1));
        assert_eq!(s.recv(ch.clone()).unwrap(), serde_json::json!(2));
        assert!(s.recv(ch).is_err());
    }
}
