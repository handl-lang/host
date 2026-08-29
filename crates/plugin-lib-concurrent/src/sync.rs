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

use handl_plugin_vm_values_json::{Context, Error, FunctionValue, NonFunctionValue};
use serde_json::json;

/// A replaceable backend for the synchronization primitives:
/// `channel`, `send`, `recv`, `mutex`, `lock`, `unlock`, `rwlock`,
/// `read_lock`, `write_lock`, `semaphore`, `acquire`, and `release`.
pub trait SyncRuntime: Send + Sync {
    /// Creates a new channel, optionally bounded by `capacity`.
    fn channel(&self, capacity: Option<NonFunctionValue>) -> NonFunctionValue;

    /// Sends `value` on `channel`.
    fn send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Tries to send `value` on `channel` without blocking.
    fn try_send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Receives a value from `channel`.
    fn recv(&self, channel: NonFunctionValue) -> Result<NonFunctionValue, SyncError>;

    /// Tries to receive a value from `channel` without blocking.
    fn try_recv(&self, channel: &NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        self.recv(channel.clone())
    }

    /// Polls `channels` and returns `[index, value]` for the first channel
    /// that has a value available, or `WouldBlock` if all are empty.
    fn select(&self, channels: &[NonFunctionValue]) -> Result<NonFunctionValue, SyncError>;

    /// Creates a new mutex.
    fn mutex(&self) -> NonFunctionValue;

    /// Acquires `mutex`, runs `thunk`, and releases the lock.
    fn lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Tries to acquire `mutex` and, if available, runs `thunk` without blocking.
    fn try_lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Releases `mutex`.
    fn unlock(&self, mutex: NonFunctionValue) -> Result<NonFunctionValue, SyncError>;

    /// Creates a new read-write lock.
    fn rwlock(&self) -> NonFunctionValue;

    /// Acquires a read lock on `rwlock` and runs `thunk`.
    fn read_lock(
        &self,
        rwlock: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Acquires a write lock on `rwlock` and runs `thunk`.
    fn write_lock(
        &self,
        rwlock: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Creates a new semaphore with `permits` initial permits.
    fn semaphore(&self, permits: NonFunctionValue) -> NonFunctionValue;

    /// Acquires `n` permits from `semaphore`.
    fn acquire(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Tries to acquire `n` permits from `semaphore` without blocking.
    fn try_acquire(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError>;

    /// Releases `n` permits to `semaphore`.
    fn release(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError>;
}

/// Creates a resource value of the form `{"type": <kind>, "id": <id>}`.
pub fn resource_value(kind: &str, id: u64) -> NonFunctionValue {
    NonFunctionValue::new(json!({"type": kind, "id": id}))
}

/// Parses a resource value and returns its `id` after checking its `type`.
pub fn resource_id(value: &NonFunctionValue, expected_kind: &str) -> Result<u64, SyncError> {
    let obj = value
        .as_object()
        .ok_or_else(|| SyncError::BadResource("expected a resource object".to_string()))?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::BadResource("resource missing 'type'".to_string()))?;
    if kind != expected_kind {
        return Err(SyncError::BadResource(format!(
            "expected resource type {expected_kind}, got {kind}"
        )));
    }
    obj.get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| SyncError::BadResource("resource missing numeric 'id'".to_string()))
}

/// Parses a `NonFunctionValue` as an unsigned integer.
pub fn as_u64(value: &NonFunctionValue) -> Result<u64, SyncError> {
    value
        .inner()
        .as_u64()
        .ok_or_else(|| SyncError::BadResource("expected a non-negative integer".to_string()))
}

fn poison<T>(_: std::sync::PoisonError<T>) -> SyncError {
    SyncError::Poisoned
}

/// Errors raised by a [`SyncRuntime`] backend.
///
/// These are intentionally distinct from VM errors so that the host
/// functions in [`ConcurrencyModule`](crate::ConcurrencyModule) can
/// translate a `WouldBlock` condition into an effect-style
/// [`handl_plugin_vm::Error::Operation`] for the scheduler to intercept.
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
    Other(Error),
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

impl From<Error> for SyncError {
    fn from(err: Error) -> Self {
        SyncError::Other(err)
    }
}

#[derive(Debug)]
struct Channel {
    capacity: Option<usize>,
    queue: VecDeque<NonFunctionValue>,
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
    pub fn close(&self, channel: NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        chan.closed = true;
        Ok(NonFunctionValue::default())
    }
}

impl Default for DefaultSyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncRuntime for DefaultSyncRuntime {
    fn channel(&self, capacity: Option<NonFunctionValue>) -> NonFunctionValue {
        let cap = capacity.and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.inner().as_u64().map(|c| c as usize)
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

    fn send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        if chan.closed {
            return Err(SyncError::NotFound(format!("channel {id} is closed")));
        }
        if let Some(cap) = chan.capacity {
            if chan.queue.len() >= cap {
                return Err(SyncError::WouldBlock);
            }
        }
        chan.queue.push_back(value);
        Ok(NonFunctionValue::default())
    }

    fn try_send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock().map_err(poison)?;
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);
        let mut chan = chan.lock().map_err(poison)?;
        if chan.closed {
            return Err(SyncError::NotFound(format!("channel {id} is closed")));
        }
        if let Some(cap) = chan.capacity {
            if chan.queue.len() >= cap {
                return Err(SyncError::WouldBlock);
            }
        }
        chan.queue.push_back(value);
        Ok(NonFunctionValue::default())
    }

    fn recv(&self, channel: NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
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
            Err(SyncError::NotFound(format!("channel {id} is closed")))
        } else {
            Err(SyncError::WouldBlock)
        }
    }

    fn select(&self, channels: &[NonFunctionValue]) -> Result<NonFunctionValue, SyncError> {
        for (i, channel) in channels.iter().enumerate() {
            if let Ok(value) = self.try_recv(channel) {
                return Ok(NonFunctionValue::new(json!([i, value.inner().clone()])));
            }
        }
        Err(SyncError::WouldBlock)
    }

    fn mutex(&self) -> NonFunctionValue {
        let id = self.next_id();
        let mut mutexes = self.mutexes.lock().unwrap();
        mutexes.insert(id, Arc::new(Mutex::new(false)));
        resource_value("mutex", id)
    }

    fn lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.lock().map_err(poison)?;
        if *guard {
            return Err(SyncError::BadResource("mutex already locked".to_string()));
        }
        *guard = true;
        let result = ctx.eval_expr(&thunk.body, &[])?;
        *guard = false;
        drop(guard);
        Ok(result)
    }

    fn try_lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.try_lock().map_err(|_| SyncError::WouldBlock)?;
        if *guard {
            return Err(SyncError::WouldBlock);
        }
        *guard = true;
        let result = ctx.eval_expr(&thunk.body, &[])?;
        *guard = false;
        Ok(result)
    }

    fn unlock(&self, mutex: NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock().map_err(poison)?;
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);
        let mut guard = m.lock().map_err(poison)?;
        if !*guard {
            return Err(SyncError::BadResource("mutex not locked".to_string()));
        }
        *guard = false;
        Ok(NonFunctionValue::default())
    }

    fn rwlock(&self) -> NonFunctionValue {
        let id = self.next_id();
        let mut rwlocks = self.rwlocks.lock().unwrap();
        rwlocks.insert(id, Arc::new(RwLock::new(())));
        resource_value("rwlock", id)
    }

    fn read_lock(
        &self,
        rwlock: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
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
        rwlock: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
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

    fn semaphore(&self, permits: NonFunctionValue) -> NonFunctionValue {
        let id = self.next_id();
        let count = as_u64(&permits).unwrap_or(0);
        let mut semaphores = self.semaphores.lock().unwrap();
        semaphores.insert(id, Arc::new(Mutex::new(count)));
        resource_value("semaphore", id)
    }

    fn acquire(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
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
            return Err(SyncError::WouldBlock);
        }
        *permits -= n;
        Ok(NonFunctionValue::default())
    }

    fn try_acquire(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
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
            return Err(SyncError::WouldBlock);
        }
        *permits -= n;
        Ok(NonFunctionValue::default())
    }

    fn release(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
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
        Ok(NonFunctionValue::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_vm_values_json::{CanonicalContext, Eval};
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
    fn channel_send_recv() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(None);

        let value = NonFunctionValue::new(json!(42));
        assert!(s.send(ch.clone(), value).is_ok());

        assert_eq!(
            s.recv(ch.clone()).unwrap(),
            NonFunctionValue::new(json!(42))
        );
        assert_eq!(s.recv(ch), Err(SyncError::WouldBlock));
    }

    #[test]
    fn bounded_channel_send_full_blocks() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(2))).is_ok());
        assert_eq!(
            s.send(ch.clone(), NonFunctionValue::new(json!(3))),
            Err(SyncError::WouldBlock)
        );
    }

    #[test]
    fn channel_recv_empty_blocks() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(None);
        assert_eq!(s.recv(ch), Err(SyncError::WouldBlock));
    }

    #[test]
    fn bounded_channel_round_trip() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(2))).is_ok());
        assert_eq!(
            s.send(ch.clone(), NonFunctionValue::new(json!(3))),
            Err(SyncError::WouldBlock)
        );

        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(1)));
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(3))).is_ok());
        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(2)));
        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(3)));
        assert_eq!(s.recv(ch), Err(SyncError::WouldBlock));
    }

    #[test]
    fn mutex_lock_unlock() {
        let s = DefaultSyncRuntime::new();
        let m = s.mutex();

        let result = s.lock(m.clone(), thunk("locked!"), &mut ctx()).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "locked!" })));

        // lock auto-releases, so an explicit unlock on an unlocked mutex errors
        assert!(s.unlock(m).is_err());
    }

    #[test]
    fn semaphore_acquire_release() {
        let s = DefaultSyncRuntime::new();
        let sem = s.semaphore(NonFunctionValue::new(json!(3)));

        assert!(
            s.acquire(sem.clone(), NonFunctionValue::new(json!(2)))
                .is_ok()
        );
        assert!(
            s.acquire(sem.clone(), NonFunctionValue::new(json!(2)))
                .is_err()
        );
        assert!(
            s.release(sem.clone(), NonFunctionValue::new(json!(2)))
                .is_ok()
        );
        assert!(s.acquire(sem, NonFunctionValue::new(json!(2))).is_ok());
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
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(2))).is_ok());
        assert_eq!(
            s.try_send(ch.clone(), NonFunctionValue::new(json!(3))),
            Err(SyncError::WouldBlock)
        );
        assert_eq!(s.recv(ch).unwrap(), NonFunctionValue::new(json!(1)));
    }

    #[test]
    fn try_acquire_insufficient_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let sem = s.semaphore(NonFunctionValue::new(json!(2)));

        assert!(
            s.try_acquire(sem.clone(), NonFunctionValue::new(json!(1)))
                .is_ok()
        );
        assert!(
            s.try_acquire(sem.clone(), NonFunctionValue::new(json!(1)))
                .is_ok()
        );
        assert_eq!(
            s.try_acquire(sem, NonFunctionValue::new(json!(1))),
            Err(SyncError::WouldBlock)
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
                Err(SyncError::WouldBlock)
            );
            *guard = false;
        }

        assert_eq!(
            s.try_lock(m, thunk("ok"), &mut ctx()).unwrap(),
            NonFunctionValue::new(json!({ "body": "ok" }))
        );
    }

    #[test]
    fn bounded_channel_blocks_on_second_send() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(1))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert_eq!(
            s.send(ch.clone(), NonFunctionValue::new(json!(2))),
            Err(SyncError::WouldBlock)
        );
        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(1)));
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(2))).is_ok());
        assert_eq!(s.recv(ch).unwrap(), NonFunctionValue::new(json!(2)));
    }

    #[test]
    fn bounded_channel_try_send_full_returns_would_block() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(1))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert_eq!(
            s.try_send(ch, NonFunctionValue::new(json!(2))),
            Err(SyncError::WouldBlock)
        );
    }

    #[test]
    fn bounded_channel_close_then_send_error() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert!(s.close(ch.clone()).is_ok());
        assert!(s.send(ch, NonFunctionValue::new(json!(2))).is_err());
    }

    #[test]
    fn bounded_channel_close_then_recv_drains_then_error() {
        let s = DefaultSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(1))).is_ok());
        assert!(s.send(ch.clone(), NonFunctionValue::new(json!(2))).is_ok());
        assert!(s.close(ch.clone()).is_ok());
        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(1)));
        assert_eq!(s.recv(ch.clone()).unwrap(), NonFunctionValue::new(json!(2)));
        assert!(s.recv(ch).is_err());
    }
}
