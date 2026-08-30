use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

use crate::error::OsError;

/// Abstract environment variable service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::env`
/// - [`env_stub::StubEnv`] — in-memory for testing
pub trait Env: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, OsError>;
    fn has(&self, key: &str) -> Result<bool, OsError>;
    fn keys(&self) -> Result<Vec<String>, OsError>;
}

/// Module adapter for [`Env`] — exposes `env.*` functions.
pub struct EnvModule {
    get: GetFn,
    has: HasFn,
    keys: KeysFn,
}

impl EnvModule {
    pub fn new(env: impl Env + 'static) -> Self {
        let env = Arc::new(env);
        EnvModule {
            get: GetFn { env: env.clone() },
            has: HasFn { env: env.clone() },
            keys: KeysFn { env },
        }
    }
}

impl Module for EnvModule {
    fn name(&self) -> &str {
        "env"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "get" => Some(&self.get),
            "has" => Some(&self.has),
            "keys" => Some(&self.keys),
            _ => None,
        }
    }
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}

struct GetFn {
    env: Arc<dyn Env>,
}

impl Function for GetFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        let value = self.env.get(key)?;
        Ok(match value {
            Some(v) => serde_json::Value::from(v),
            None => serde_json::Value::Null,
        })
    }
}

struct HasFn {
    env: Arc<dyn Env>,
}

impl Function for HasFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        self.env
            .has(key)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct KeysFn {
    env: Arc<dyn Env>,
}

impl Function for KeysFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let keys = self.env.keys()?;
        let arr: Vec<serde_json::Value> = keys.into_iter().map(serde_json::Value::String).collect();
        Ok(serde_json::Value::from(serde_json::Value::Array(arr)))
    }
}
