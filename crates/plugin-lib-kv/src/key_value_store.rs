use handl_platform::{EvalContext, EvalError, Function};
use std::sync::Arc;

use handl_platform::Module;
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::KvError;

/// The handl source declaring the kv `op` interface — the single source
/// of truth for the kv API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const KV_HNL: &str = include_str!("../../../handl-packages/kv/kv.hnl");

/// The op names declared in `kv.hnl`.
const KV_OPS: &[&str] = &[
    "kv.get",
    "kv.set",
    "kv.remove",
    "kv.clear",
    "kv.has",
    "kv.size",
];

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
///
/// The interface is declared in `kv.hnl` via `cap` declarations and loaded
/// via the generic [`HandlModule`]; this struct delegates [`Module`] lookups
/// to the [`HandlModule`] built from the store.
pub struct KeyValueStoreModule {
    module: HandlModule,
}

impl KeyValueStoreModule {
    /// Creates a `KeyValueStoreModule` with the given [`KeyValueStore`].
    pub fn new(store: impl KeyValueStore + 'static) -> Self {
        let registry = Arc::new(kv_op_registry(store));
        Self {
            module: HandlModule::new("kv", KV_HNL, &[]).with_ops(KV_OPS, registry),
        }
    }
}

impl Module for KeyValueStoreModule {
    fn name(&self) -> &str {
        "kv"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
    }
}

/// Build an [`OpRegistry`] for the kv ops, registering Rust handls that
/// delegate to the given [`KeyValueStore`] under their op names.
fn kv_op_registry(store: impl KeyValueStore + 'static) -> OpRegistry {
    let store: Arc<dyn KeyValueStore> = Arc::new(store);
    let mut registry = OpRegistry::new();
    registry.register(
        "kv.get",
        Arc::new(GetFn {
            store: store.clone(),
        }),
    );
    registry.register(
        "kv.set",
        Arc::new(SetFn {
            store: store.clone(),
        }),
    );
    registry.register(
        "kv.remove",
        Arc::new(RemoveFn {
            store: store.clone(),
        }),
    );
    registry.register(
        "kv.clear",
        Arc::new(ClearFn {
            store: store.clone(),
        }),
    );
    registry.register(
        "kv.has",
        Arc::new(HasFn {
            store: store.clone(),
        }),
    );
    registry.register("kv.size", Arc::new(SizeFn { store }));
    registry
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1, 0,
    )))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let key = args[0].as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))?;
    let value = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))?;
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
