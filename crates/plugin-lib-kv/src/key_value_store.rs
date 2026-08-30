use std::sync::Arc;
use handl_platform::{EvalContext, EvalError, Function};

use handl_platform::Module;

use crate::error::KvError;

/// Abstract key-value store service.
///
/// Implementations:
/// - [`crate::StubKeyValueStore`] — in-memory `HashMap` for testing
/// - Future backends: sled, SQLite, Redis, etc.
pub trait KeyValueStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, KvError>;
    fn set(&self, key: &str, value: &str) -> Result<(), KvError>;
    fn remove(&self, key: &str) -> Result<(), KvError>;
    fn clear(&self) -> Result<(), KvError>;
    fn has(&self, key: &str) -> Result<bool, KvError>;
    fn size(&self) -> Result<usize, KvError>;
}

/// Module adapter for [`KeyValueStore`] — exposes `kv.*` functions.
pub struct KeyValueStoreModule {
    get: GetFn,
    set: SetFn,
    remove: RemoveFn,
    clear: ClearFn,
    has: HasFn,
    size: SizeFn,
}

impl KeyValueStoreModule {
    pub fn new(store: impl KeyValueStore + 'static) -> Self {
        let store = Arc::new(store);
        KeyValueStoreModule {
            get: GetFn {
                store: store.clone(),
            },
            set: SetFn {
                store: store.clone(),
            },
            remove: RemoveFn {
                store: store.clone(),
            },
            clear: ClearFn {
                store: store.clone(),
            },
            has: HasFn {
                store: store.clone(),
            },
            size: SizeFn { store },
        }
    }
}

impl Module for KeyValueStoreModule {
    fn name(&self) -> &str {
        "kv"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "get" => Some(&self.get),
            "set" => Some(&self.set),
            "remove" => Some(&self.remove),
            "clear" => Some(&self.clear),
            "has" => Some(&self.has),
            "size" => Some(&self.size),
            _ => None,
        }
    }
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}

fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let key = args[0].as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))?;
    let value = args[1].as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))?;
    Ok((key, value))
}

struct GetFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for GetFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        let value = self.store.get(key)?;
        Ok(match value {
            Some(v) => serde_json::Value::from(v),
            None => serde_json::Value::Null,
        })
    }
}

struct SetFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for SetFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (key, value) = require_two_strings(args)?;
        self.store.set(key, value)?;
        Ok(serde_json::Value::Null)
    }
}

struct RemoveFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for RemoveFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        self.store.remove(key)?;
        Ok(serde_json::Value::Null)
    }
}

struct ClearFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for ClearFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.store.clear()?;
        Ok(serde_json::Value::Null)
    }
}

struct HasFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for HasFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        self.store
            .has(key)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct SizeFn {
    store: Arc<dyn KeyValueStore>,
}

impl Function for SizeFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.store
            .size()
            .map(|n| serde_json::Value::from(serde_json::json!(n)))
            .map_err(Into::into)
    }
}
