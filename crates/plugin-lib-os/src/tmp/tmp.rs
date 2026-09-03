use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;

use crate::error::OsError;

/// Abstract temporary file/directory service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::env::temp_dir`
/// - [`tmp_stub::StubTmp`] — canned paths for testing
pub trait Tmp: Send + Sync {
    fn file(&self) -> Result<String, OsError>;
    fn dir(&self) -> Result<String, OsError>;
}

/// Module adapter for [`Tmp`] — exposes `tmp.*` functions.
pub struct TmpModule {
    file: FileFn,
    dir: DirFn,
}

impl TmpModule {
    pub fn new(tmp: impl Tmp + 'static) -> Self {
        let tmp = Arc::new(tmp);
        TmpModule {
            file: FileFn { tmp: tmp.clone() },
            dir: DirFn { tmp },
        }
    }
}

impl Module for TmpModule {
    fn name(&self) -> &str {
        "tmp"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "file" => Some(&self.file),
            "dir" => Some(&self.dir),
            _ => None,
        }
    }
}

struct FileFn {
    tmp: Arc<dyn Tmp>,
}

impl Function for FileFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.tmp
            .file()
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct DirFn {
    tmp: Arc<dyn Tmp>,
}

impl Function for DirFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.tmp
            .dir()
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `tmp` module.
pub const TMP_OPS: &[&str] = &[
    "tmp.file",
    "tmp.dir",
];

/// Register the `tmp` ops into the given [`OpRegistry`], dispatching to the
/// given [`Tmp`] implementation.
pub fn register_tmp_ops(registry: &mut OpRegistry, tmp: impl Tmp + 'static) {
    let tmp: Arc<dyn Tmp> = Arc::new(tmp);
    registry.register("tmp.file", Arc::new(FileFn { tmp: tmp.clone() }));
    registry.register("tmp.dir", Arc::new(DirFn { tmp }));
}
