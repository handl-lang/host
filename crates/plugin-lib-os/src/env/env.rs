use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

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
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct EnvModule {
    module: HandlModule,
}

impl EnvModule {
    pub fn new(env: impl Env + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_env_ops(&mut registry, env);
        Self {
            module: HandlModule::new("env", OS_HNL, &[]).with_ops(ENV_OPS, Arc::new(registry)),
        }
    }
}

impl Module for EnvModule {
    fn name(&self) -> &str {
        self.module.name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
    }
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

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `env` module.
pub const ENV_OPS: &[&str] = &["env.get", "env.has", "env.keys"];

/// Register the `env` ops into the given [`OpRegistry`], dispatching to the
/// given [`Env`] implementation.
pub fn register_env_ops(registry: &mut OpRegistry, env: impl Env + 'static) {
    let env: Arc<dyn Env> = Arc::new(env);
    registry.register("env.get", Arc::new(GetFn { env: env.clone() }));
    registry.register("env.has", Arc::new(HasFn { env: env.clone() }));
    registry.register("env.keys", Arc::new(KeysFn { env }));
}
