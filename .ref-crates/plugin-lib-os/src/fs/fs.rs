use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

/// Abstract filesystem service.
///
/// Defines file I/O operations without tying code to a concrete platform.
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::fs`
/// - [`fs_stub::StubFs`] — in-memory for testing
pub trait Fs: Send + Sync {
    fn read_file(&self, path: &str) -> Result<String, OsError>;
    fn write_file(&self, path: &str, content: &str) -> Result<(), OsError>;
    fn exists(&self, path: &str) -> Result<bool, OsError>;
    fn remove(&self, path: &str) -> Result<(), OsError>;
    fn list_dir(&self, path: &str) -> Result<Vec<String>, OsError>;
}

/// Module adapter for [`Fs`] — exposes `fs.*` functions.
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct FsModule {
    module: HandlModule,
}

impl FsModule {
    pub fn new(fs: impl Fs + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_fs_ops(&mut registry, fs);
        Self {
            module: HandlModule::new("fs", OS_HNL, &[]).with_ops(FS_OPS, Arc::new(registry)),
        }
    }
}

impl Module for FsModule {
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

fn require_string_arg(args: &[serde_json::Value], expected: usize) -> Result<Vec<&str>, EvalError> {
    if args.len() < expected {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            expected,
            args.len()
        )));
    }
    args.iter()
        .take(expected)
        .map(|v| {
            v.as_str().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "string", "non-string",
            )))
        })
        .collect()
}

struct ReadFileFn {
    fs: Arc<dyn Fs>,
}

impl Function for ReadFileFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_string_arg(args, 1)?[0];
        self.fs
            .read_file(path)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct WriteFileFn {
    fs: Arc<dyn Fs>,
}

impl Function for WriteFileFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_string_arg(args, 2)?[0];
        let content = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
        self.fs.write_file(path, content)?;
        Ok(serde_json::Value::Null)
    }
}

struct ExistsFn {
    fs: Arc<dyn Fs>,
}

impl Function for ExistsFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_string_arg(args, 1)?[0];
        self.fs
            .exists(path)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct RemoveFn {
    fs: Arc<dyn Fs>,
}

impl Function for RemoveFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_string_arg(args, 1)?[0];
        self.fs.remove(path)?;
        Ok(serde_json::Value::Null)
    }
}

struct ListDirFn {
    fs: Arc<dyn Fs>,
}

impl Function for ListDirFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_string_arg(args, 1)?[0];
        let entries = self.fs.list_dir(path)?;
        let arr: Vec<serde_json::Value> =
            entries.into_iter().map(serde_json::Value::String).collect();
        Ok(serde_json::Value::from(serde_json::Value::Array(arr)))
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `fs` module.
pub const FS_OPS: &[&str] = &["fs.read", "fs.write", "fs.exists", "fs.remove", "fs.list"];

/// Register the `fs` ops into the given [`OpRegistry`], dispatching to the
/// given [`Fs`] implementation.
pub fn register_fs_ops(registry: &mut OpRegistry, fs: impl Fs + 'static) {
    let fs: Arc<dyn Fs> = Arc::new(fs);
    registry.register("fs.read", Arc::new(ReadFileFn { fs: fs.clone() }));
    registry.register("fs.write", Arc::new(WriteFileFn { fs: fs.clone() }));
    registry.register("fs.exists", Arc::new(ExistsFn { fs: fs.clone() }));
    registry.register("fs.remove", Arc::new(RemoveFn { fs: fs.clone() }));
    registry.register("fs.list", Arc::new(ListDirFn { fs }));
}
