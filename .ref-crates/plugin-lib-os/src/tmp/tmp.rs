use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

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
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct TmpModule {
    module: HandlModule,
}

impl TmpModule {
    pub fn new(tmp: impl Tmp + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_tmp_ops(&mut registry, tmp);
        Self {
            module: HandlModule::new("tmp", OS_HNL, &[]).with_ops(TMP_OPS, Arc::new(registry)),
        }
    }
}

impl Module for TmpModule {
    fn name(&self) -> &str {
        "tmp"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
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
pub const TMP_OPS: &[&str] = &["tmp.file", "tmp.dir"];

/// Register the `tmp` ops into the given [`OpRegistry`], dispatching to the
/// given [`Tmp`] implementation.
pub fn register_tmp_ops(registry: &mut OpRegistry, tmp: impl Tmp + 'static) {
    let tmp: Arc<dyn Tmp> = Arc::new(tmp);
    registry.register("tmp.file", Arc::new(FileFn { tmp: tmp.clone() }));
    registry.register("tmp.dir", Arc::new(DirFn { tmp }));
}
