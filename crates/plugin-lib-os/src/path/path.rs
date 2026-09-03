use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;

use crate::error::OsError;

/// Abstract path manipulation service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::path`
/// - [`path_stub::StubPath`] — POSIX-like for testing
pub trait Path: Send + Sync {
    fn join(&self, parts: &[&str]) -> Result<String, OsError>;
    fn resolve(&self, parts: &[&str]) -> Result<String, OsError>;
    fn basename(&self, path: &str) -> Result<String, OsError>;
    fn dirname(&self, path: &str) -> Result<String, OsError>;
    fn extname(&self, path: &str) -> Result<String, OsError>;
}

/// Module adapter for [`Path`] — exposes `path.*` functions.
pub struct PathModule {
    join: JoinFn,
    resolve: ResolveFn,
    basename: BasenameFn,
    dirname: DirnameFn,
    extname: ExtnameFn,
}

impl PathModule {
    pub fn new(path: impl Path + 'static) -> Self {
        let path = Arc::new(path);
        PathModule {
            join: JoinFn { path: path.clone() },
            resolve: ResolveFn { path: path.clone() },
            basename: BasenameFn { path: path.clone() },
            dirname: DirnameFn { path: path.clone() },
            extname: ExtnameFn { path },
        }
    }
}

impl Module for PathModule {
    fn name(&self) -> &str {
        "path"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "join" => Some(&self.join),
            "resolve" => Some(&self.resolve),
            "basename" => Some(&self.basename),
            "dirname" => Some(&self.dirname),
            "extname" => Some(&self.extname),
            _ => None,
        }
    }
}

fn collect_string_args(args: &[serde_json::Value]) -> Result<Vec<&str>, EvalError> {
    if args.is_empty() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )));
    }
    args.iter()
        .map(|v| {
            v.as_str().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "string", "non-string",
            )))
        })
        .collect()
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

struct JoinFn {
    path: Arc<dyn Path>,
}

impl Function for JoinFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let parts = collect_string_args(args)?;
        self.path
            .join(&parts)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct ResolveFn {
    path: Arc<dyn Path>,
}

impl Function for ResolveFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let parts = collect_string_args(args)?;
        self.path
            .resolve(&parts)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct BasenameFn {
    path: Arc<dyn Path>,
}

impl Function for BasenameFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_one_string(args)?;
        self.path
            .basename(path)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct DirnameFn {
    path: Arc<dyn Path>,
}

impl Function for DirnameFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_one_string(args)?;
        self.path
            .dirname(path)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct ExtnameFn {
    path: Arc<dyn Path>,
}

impl Function for ExtnameFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let path = require_one_string(args)?;
        self.path
            .extname(path)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `path` module.
pub const PATH_OPS: &[&str] = &[
    "path.join",
    "path.resolve",
    "path.basename",
    "path.dirname",
    "path.extname",
];

/// Register the `path` ops into the given [`OpRegistry`], dispatching to the
/// given [`Path`] implementation.
pub fn register_path_ops(registry: &mut OpRegistry, path: impl Path + 'static) {
    let path: Arc<dyn Path> = Arc::new(path);
    registry.register("path.join", Arc::new(JoinFn { path: path.clone() }));
    registry.register("path.resolve", Arc::new(ResolveFn { path: path.clone() }));
    registry.register("path.basename", Arc::new(BasenameFn { path: path.clone() }));
    registry.register("path.dirname", Arc::new(DirnameFn { path: path.clone() }));
    registry.register("path.extname", Arc::new(ExtnameFn { path }));
}
