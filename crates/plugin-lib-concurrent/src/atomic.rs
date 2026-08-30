//! Atomic values backend for `handl-lib-concurrent`.
//!
//! The [`AtomicRuntime`] trait is a **replaceable backend** for the atomic
//! primitives exposed by [`ConcurrencyModule`]: `atomic`, `atomic_read`,
//! `atomic_write`, and `atomic_cas`. [`DefaultAtomicRuntime`] is the bundled,
//! in-memory, `std`-based default.
//!
//! A host builder can swap the atomic backend by providing any
//! `Arc<dyn AtomicRuntime + Send + Sync>`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use handl_platform::EvalError;
use serde_json::Value;
use serde_json::json;

/// A replaceable backend for the atomic value primitives:
/// `atomic`, `atomic_read`, `atomic_write`, and `atomic_cas`.
pub trait AtomicRuntime {
    /// Creates a new atomic cell initialized to `initial` and returns its
    /// resource value.
    fn atomic(&self, initial: Value) -> Result<Value, EvalError>;

    /// Reads the current value of an atomic cell.
    fn atomic_read(&self, r: Value) -> Result<Value, EvalError>;

    /// Writes `value` to an atomic cell and returns the previous value.
    fn atomic_write(
        &self,
        r: Value,
        value: Value,
    ) -> Result<Value, EvalError>;

    /// Compares-and-swaps `new` into the cell if it currently equals `expected`.
    /// Returns a JSON boolean (`true` for success, `false` for failure).
    fn atomic_cas(
        &self,
        r: Value,
        expected: Value,
        new: Value,
    ) -> Result<Value, EvalError>;
}

fn resource_value(kind: &str, id: u64) -> Value {
    serde_json::json!(json!({"type": kind, "id": id}))
}

fn resource_id(value: &Value, expected_kind: &str) -> Result<u64, EvalError> {
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

fn poison<T>(_: std::sync::PoisonError<T>) -> EvalError {
    EvalError::Other("atomic runtime lock poisoned".to_string())
}

/// An in-memory, `std`-based atomic value runtime.
#[derive(Debug)]
pub struct DefaultAtomicRuntime {
    next_id: AtomicU64,
    cells: Mutex<HashMap<u64, Value>>,
}

impl DefaultAtomicRuntime {
    /// Creates a new empty atomic runtime.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            cells: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for DefaultAtomicRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicRuntime for DefaultAtomicRuntime {
    fn atomic(&self, initial: Value) -> Result<Value, EvalError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.cells.lock().map_err(poison)?.insert(id, initial);
        Ok(resource_value("atomic", id))
    }

    fn atomic_read(&self, r: Value) -> Result<Value, EvalError> {
        let id = resource_id(&r, "atomic")?;
        let cells = self.cells.lock().map_err(poison)?;
        cells
            .get(&id)
            .cloned()
            .ok_or_else(|| EvalError::Other(format!("atomic {id} not found")))
    }

    fn atomic_write(
        &self,
        r: Value,
        value: Value,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&r, "atomic")?;
        let mut cells = self.cells.lock().map_err(poison)?;
        let old = cells.get(&id).cloned().unwrap_or_default();
        cells.insert(id, value);
        Ok(old)
    }

    fn atomic_cas(
        &self,
        r: Value,
        expected: Value,
        new: Value,
    ) -> Result<Value, EvalError> {
        let id = resource_id(&r, "atomic")?;
        let mut cells = self.cells.lock().map_err(poison)?;
        let entry = cells
            .get_mut(&id)
            .ok_or_else(|| EvalError::Other(format!("atomic {id} not found")))?;
        if *entry == expected {
            *entry = new;
            Ok(serde_json::json!(json!(true)))
        } else {
            Ok(serde_json::json!(json!(false)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn atomic_lifecycle() {
        let a = DefaultAtomicRuntime::new();

        let r = a.atomic(serde_json::json!(json!(0))).unwrap();
        assert_eq!(r.get("type"), Some(&json!("atomic")));

        assert_eq!(
            a.atomic_read(r.clone()).unwrap(),
            serde_json::json!(json!(0))
        );

        let old = a
            .atomic_write(r.clone(), serde_json::json!(json!(42)))
            .unwrap();
        assert_eq!(old, serde_json::json!(json!(0)));

        assert_eq!(
            a.atomic_read(r.clone()).unwrap(),
            serde_json::json!(json!(42))
        );

        let swapped = a
            .atomic_cas(
                r.clone(),
                serde_json::json!(json!(42)),
                serde_json::json!(json!(7)),
            )
            .unwrap();
        assert_eq!(swapped, serde_json::json!(json!(true)));
        assert_eq!(
            a.atomic_read(r.clone()).unwrap(),
            serde_json::json!(json!(7))
        );

        let failed = a
            .atomic_cas(
                r,
                serde_json::json!(json!(42)),
                serde_json::json!(json!(9)),
            )
            .unwrap();
        assert_eq!(failed, serde_json::json!(json!(false)));
    }
}
