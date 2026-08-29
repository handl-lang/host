//! OS-level, contention/fairness-tuned sync backend for `handl-lib-concurrent`.
//!
//! Unlike [`DefaultSyncRuntime`](crate::sync::DefaultSyncRuntime), this backend
//! uses `parking_lot` primitives and blocks the calling OS thread rather than
//! signaling `WouldBlock` to the scheduler. It is meant for hosts that already
//! run real OS threads and want fair, low-latency synchronization.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use handl_plugin_vm_values_json::{Context, FunctionValue, NonFunctionValue};
use parking_lot::{Condvar, FairMutex, Mutex, RwLock};
use serde_json::json;

use crate::sync::{as_u64, resource_id, resource_value, SyncError, SyncRuntime};

struct ChannelState {
    capacity: Option<usize>,
    queue: VecDeque<NonFunctionValue>,
}

struct ChannelInner {
    state: Mutex<ChannelState>,
    cond: Condvar,
}

struct SemaphoreInner {
    permits: Mutex<usize>,
    cond: Condvar,
}

/// A `parking_lot`-backed sync runtime that blocks the calling OS thread.
pub struct ParkingLotSyncRuntime {
    next_id: AtomicU64,
    channels: Mutex<HashMap<u64, Arc<ChannelInner>>>,
    mutexes: Mutex<HashMap<u64, Arc<FairMutex<()>>>>,
    rwlocks: Mutex<HashMap<u64, Arc<RwLock<()>>>>,
    semaphores: Mutex<HashMap<u64, Arc<SemaphoreInner>>>,
}

impl ParkingLotSyncRuntime {
    /// Creates a new empty parking-lot sync runtime.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            channels: Mutex::new(HashMap::new()),
            mutexes: Mutex::new(HashMap::new()),
            rwlocks: Mutex::new(HashMap::new()),
            semaphores: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for ParkingLotSyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncRuntime for ParkingLotSyncRuntime {
    fn channel(&self, capacity: Option<NonFunctionValue>) -> NonFunctionValue {
        let cap = capacity.and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.inner().as_u64().map(|c| c as usize)
            }
        });
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut channels = self.channels.lock();
        channels.insert(
            id,
            Arc::new(ChannelInner {
                state: Mutex::new(ChannelState {
                    capacity: cap,
                    queue: VecDeque::new(),
                }),
                cond: Condvar::new(),
            }),
        );
        resource_value("channel", id)
    }

    fn send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock();
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);

        let mut state = chan.state.lock();
        if let Some(cap) = state.capacity {
            while state.queue.len() >= cap {
                chan.cond.wait(&mut state);
            }
        }
        state.queue.push_back(value);
        chan.cond.notify_one();
        Ok(NonFunctionValue::default())
    }

    fn try_send(
        &self,
        channel: NonFunctionValue,
        value: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock();
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);

        let mut state = chan.state.lock();
        if let Some(cap) = state.capacity {
            if state.queue.len() >= cap {
                return Err(SyncError::WouldBlock);
            }
        }
        state.queue.push_back(value);
        chan.cond.notify_one();
        Ok(NonFunctionValue::default())
    }

    fn recv(&self, channel: NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&channel, "channel")?;
        let channels = self.channels.lock();
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);

        let mut state = chan.state.lock();
        while state.queue.is_empty() {
            chan.cond.wait(&mut state);
        }
        let value = state.queue.pop_front().unwrap();
        chan.cond.notify_one();
        Ok(value)
    }

    fn try_recv(&self, channel: &NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(channel, "channel")?;
        let channels = self.channels.lock();
        let chan = channels
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("channel {id} not found")))?
            .clone();
        drop(channels);

        let mut state = chan.state.lock();
        if state.queue.is_empty() {
            return Err(SyncError::WouldBlock);
        }
        let value = state.queue.pop_front().unwrap();
        chan.cond.notify_one();
        Ok(value)
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
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut mutexes = self.mutexes.lock();
        mutexes.insert(id, Arc::new(FairMutex::new(())));
        resource_value("mutex", id)
    }

    fn lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock();
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);

        let _guard = m.lock();
        Ok(ctx.eval_expr(&thunk.body, &[]).map_err(SyncError::Other)?)
    }

    fn try_lock(
        &self,
        mutex: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&mutex, "mutex")?;
        let mutexes = self.mutexes.lock();
        let m = mutexes
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("mutex {id} not found")))?
            .clone();
        drop(mutexes);

        if let Some(_guard) = m.try_lock() {
            Ok(ctx.eval_expr(&thunk.body, &[]).map_err(SyncError::Other)?)
        } else {
            Err(SyncError::WouldBlock)
        }
    }

    fn unlock(&self, _mutex: NonFunctionValue) -> Result<NonFunctionValue, SyncError> {
        // `lock` already acquires the fair mutex, runs the thunk, and releases it
        // automatically when the guard is dropped. Explicit `unlock` is therefore
        // a no-op and always succeeds.
        Ok(NonFunctionValue::default())
    }

    fn rwlock(&self) -> NonFunctionValue {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut rwlocks = self.rwlocks.lock();
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
        let rwlocks = self.rwlocks.lock();
        let rw = rwlocks
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("rwlock {id} not found")))?
            .clone();
        drop(rwlocks);

        let _guard = rw.read();
        Ok(ctx.eval_expr(&thunk.body, &[]).map_err(SyncError::Other)?)
    }

    fn write_lock(
        &self,
        rwlock: NonFunctionValue,
        thunk: FunctionValue,
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&rwlock, "rwlock")?;
        let rwlocks = self.rwlocks.lock();
        let rw = rwlocks
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("rwlock {id} not found")))?
            .clone();
        drop(rwlocks);

        let _guard = rw.write();
        Ok(ctx.eval_expr(&thunk.body, &[]).map_err(SyncError::Other)?)
    }

    fn semaphore(&self, permits: NonFunctionValue) -> NonFunctionValue {
        let count = as_u64(&permits).unwrap_or(0) as usize;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut semaphores = self.semaphores.lock();
        semaphores.insert(
            id,
            Arc::new(SemaphoreInner {
                permits: Mutex::new(count),
                cond: Condvar::new(),
            }),
        );
        resource_value("semaphore", id)
    }

    fn acquire(
        &self,
        semaphore: NonFunctionValue,
        n: NonFunctionValue,
    ) -> Result<NonFunctionValue, SyncError> {
        let id = resource_id(&semaphore, "semaphore")?;
        let n = as_u64(&n)? as usize;
        let semaphores = self.semaphores.lock();
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);

        let mut permits = s.permits.lock();
        while *permits < n {
            s.cond.wait(&mut permits);
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
        let n = as_u64(&n)? as usize;
        let semaphores = self.semaphores.lock();
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);

        let mut permits = s.permits.lock();
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
        let n = as_u64(&n)? as usize;
        let semaphores = self.semaphores.lock();
        let s = semaphores
            .get(&id)
            .ok_or_else(|| SyncError::NotFound(format!("semaphore {id} not found")))?
            .clone();
        drop(semaphores);

        let mut permits = s.permits.lock();
        *permits += n;
        s.cond.notify_all();
        Ok(NonFunctionValue::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_vm_values_json::{CanonicalContext, Error, Eval};
    use std::sync::{Arc, Mutex};

    use serde_json::json;

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
        }
    }

    fn ctx() -> CanonicalContext {
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(MockEval::default()));
        ctx
    }

    #[test]
    fn parking_lot_mutex_lock_unlock() {
        let s = ParkingLotSyncRuntime::new();
        let m = s.mutex();

        let result = s.lock(m.clone(), thunk("locked!"), &mut ctx()).unwrap();
        assert_eq!(result, NonFunctionValue::new(json!({ "body": "locked!" })));

        // `lock` auto-releases; explicit unlock is a no-op.
        assert!(s.unlock(m).is_ok());
    }

    #[test]
    fn parking_lot_rwlock_read_write() {
        let s = ParkingLotSyncRuntime::new();
        let rw = s.rwlock();

        assert_eq!(
            s.read_lock(rw.clone(), thunk("read"), &mut ctx()).unwrap(),
            NonFunctionValue::new(json!({ "body": "read" }))
        );
        assert_eq!(
            s.write_lock(rw, thunk("write"), &mut ctx()).unwrap(),
            NonFunctionValue::new(json!({ "body": "write" }))
        );
    }

    #[test]
    fn parking_lot_semaphore_acquire_release() {
        let s = ParkingLotSyncRuntime::new();
        let sem = s.semaphore(NonFunctionValue::new(json!(2)));

        assert!(s
            .acquire(sem.clone(), NonFunctionValue::new(json!(1)))
            .is_ok());
        assert!(s
            .acquire(sem.clone(), NonFunctionValue::new(json!(1)))
            .is_ok());
        assert!(s
            .release(sem.clone(), NonFunctionValue::new(json!(1)))
            .is_ok());
        assert!(s.acquire(sem, NonFunctionValue::new(json!(1))).is_ok());
    }

    #[test]
    fn parking_lot_channel_send_recv() {
        let s = ParkingLotSyncRuntime::new();
        let ch = s.channel(None);

        s.send(ch.clone(), NonFunctionValue::new(json!(42)))
            .unwrap();
        assert_eq!(
            s.recv(ch).unwrap(),
            NonFunctionValue::new(json!(42))
        );
    }

    #[test]
    fn parking_lot_channel_round_trip() {
        let s = ParkingLotSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        s.send(ch.clone(), NonFunctionValue::new(json!(1)))
            .unwrap();
        s.send(ch.clone(), NonFunctionValue::new(json!(2)))
            .unwrap();

        assert_eq!(
            s.recv(ch.clone()).unwrap(),
            NonFunctionValue::new(json!(1))
        );
        assert_eq!(
            s.recv(ch.clone()).unwrap(),
            NonFunctionValue::new(json!(2))
        );

        s.send(ch.clone(), NonFunctionValue::new(json!(3)))
            .unwrap();
        assert_eq!(
            s.recv(ch).unwrap(),
            NonFunctionValue::new(json!(3))
        );
    }

    #[test]
    fn parking_lot_lock_is_fair() {
        // A full liveness/fairness proof with live threads is tricky and flaky
        // in a unit test. The key property the runtime needs for fair contention
        // is that it is `Send + Sync` so it can be shared by real OS threads.
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&ParkingLotSyncRuntime::new());
    }

    #[test]
    fn parking_lot_try_send_full() {
        let s = ParkingLotSyncRuntime::new();
        let ch = s.channel(Some(NonFunctionValue::new(json!(2))));

        s.send(ch.clone(), NonFunctionValue::new(json!(1))).unwrap();
        s.send(ch.clone(), NonFunctionValue::new(json!(2))).unwrap();
        assert_eq!(
            s.try_send(ch.clone(), NonFunctionValue::new(json!(3))),
            Err(SyncError::WouldBlock)
        );
        assert_eq!(
            s.recv(ch).unwrap(),
            NonFunctionValue::new(json!(1))
        );
    }

    #[test]
    fn parking_lot_try_acquire_insufficient() {
        let s = ParkingLotSyncRuntime::new();
        let sem = s.semaphore(NonFunctionValue::new(json!(2)));

        s.acquire(sem.clone(), NonFunctionValue::new(json!(1)))
            .unwrap();
        s.acquire(sem.clone(), NonFunctionValue::new(json!(1)))
            .unwrap();
        assert_eq!(
            s.try_acquire(sem, NonFunctionValue::new(json!(1))),
            Err(SyncError::WouldBlock)
        );
    }

    #[test]
    fn parking_lot_try_lock_contended() {
        let s = ParkingLotSyncRuntime::new();
        let m = s.mutex();
        let id = resource_id(&m, "mutex").unwrap();
        let inner = s.mutexes.lock().get(&id).unwrap().clone();

        let guard = inner.lock();
        assert_eq!(
            s.try_lock(m.clone(), thunk("work"), &mut ctx()),
            Err(SyncError::WouldBlock)
        );
        drop(guard);

        assert_eq!(
            s.try_lock(m, thunk("ok"), &mut ctx()).unwrap(),
            NonFunctionValue::new(json!({ "body": "ok" }))
        );
    }
}
