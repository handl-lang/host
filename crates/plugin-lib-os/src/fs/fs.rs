use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

use crate::error::OsError;

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
pub struct FsModule {
    read: ReadFileFn,
    write: WriteFileFn,
    exists: ExistsFn,
    remove: RemoveFn,
    list: ListDirFn,
}

impl FsModule {
    pub fn new(fs: impl Fs + 'static) -> Self {
        let fs = Arc::new(fs);
        FsModule {
            read: ReadFileFn { fs: fs.clone() },
            write: WriteFileFn { fs: fs.clone() },
            exists: ExistsFn { fs: fs.clone() },
            remove: RemoveFn { fs: fs.clone() },
            list: ListDirFn { fs },
        }
    }
}

impl Module for FsModule {
    fn name(&self) -> &str {
        "fs"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "read" => Some(&self.read),
            "write" => Some(&self.write),
            "exists" => Some(&self.exists),
            "remove" => Some(&self.remove),
            "list" => Some(&self.list),
            _ => None,
        }
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
            v.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
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
        let content = args[1].as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))?;
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
