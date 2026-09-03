//! # handl-lib-os-std
//!
//! Concrete OS service implementations backed by the Rust stdlib.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The OS interface is declared in
//! `kit-stdlib/data/os/os.hnl`, and this crate's `handl` bindings are
//! declared in `kit-stdlib/data/os_std/os_std.hnl`. The Rust handls
//! (wrapping stdlib implementations) are registered in an [`OpRegistry`]
//! via [`os_std_op_registry`], which delegates to the shared
//! [`os_op_registry`](handl_plugin_lib_os::os_op_registry) builder. The
//! [`OsStdLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the op registry, eliminating the per-library `Plugin` trait scaffolding.
//!
//! Provides [`StdFs`] (`std::fs`), [`StdEnv`] (`std::env`),
//! [`StdPath`] (`std::path`), [`StdTime`] (`std::time`),
//! [`StdCmd`] (`std::process`), [`StdProcess`] (`std::process`),
//! [`StdTmp`] (`std::temp_dir`), [`StdSocket`] (`std::net`), [`StdSig`] (platform signals).
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_module::ModuleRuntime;
//! use handl_plugin_lib_os::{FsModule, EnvModule, PathModule, TimeModule, CmdModule, ProcessModule, TmpModule, SocketModule, SigModule};
//! use handl_plugin_lib_os_std::{StdFs, StdEnv, StdPath, StdTime, StdCmd, StdProcess, StdTmp, StdSocket, StdSig};
//!
//! let host = ModuleRuntime::new()
//!     .with(FsModule::new(StdFs))
//!     .with(EnvModule::new(StdEnv))
//!     .with(PathModule::new(StdPath))
//!     .with(TimeModule::new(StdTime))
//!     .with(CmdModule::new(StdCmd))
//!     .with(ProcessModule::new(StdProcess))
//!     .with(TmpModule::new(StdTmp))
//!     .with(SocketModule::new(StdSocket))
//!     .with(SigModule::new(StdSig));
//! ```

mod cmd;
mod env;
mod fs;
mod path;
mod process;
mod sig;
mod socket;
mod time;
mod tmp;

#[cfg(unix)]
mod mio_socket;

#[cfg(test)]
mod sig_tests;
#[cfg(test)]
mod socket_tests;

use std::sync::Arc;

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::{parse_cap_declarations, HandlLibraryPlugin, HandlModule};
use handl_plugin_lib_os::os_op_registry;
use handl_plugin_module::{ModuleRule, ModuleRuntime};

pub use cmd::StdCmd;
pub use env::StdEnv;
pub use fs::StdFs;
#[cfg(unix)]
pub use mio_socket::MioSocket;
pub use path::StdPath;
pub use process::StdProcess;
pub use sig::StdSig;
pub use socket::StdSocket;
pub use time::StdTime;
pub use tmp::StdTmp;

// Re-export OS interface types for convenience.
pub use handl_plugin_lib_os::{
    AsyncSocket, Cmd, CmdOutput, Env, Fs, OsError, Path, Process, ProcessStatus, Sig, Socket,
    Time, Tmp, OS_OPS,
};
pub use handl_plugin_lib_os::{
    CMD_OPS, ENV_OPS, FS_OPS, PATH_OPS, PROCESS_OPS, SIG_OPS, SOCKET_OPS, TIME_OPS, TMP_OPS,
};

/// The handl source declaring the os-std `handl` bindings — the
/// implementation-side declarations for the OS ops. Loaded at compile
/// time from the kit-stdlib data dir.
const OS_STD_HNL: &str = include_str!("../../../kit-stdlib/data/os_std/os_std.hnl");

/// Build the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for
/// os-std, registering the Rust handls (wrapping stdlib implementations)
/// under the op names declared in the os interface.
///
/// This delegates to the shared
/// [`os_op_registry`](handl_plugin_lib_os::os_op_registry) builder,
/// passing the std-backed service implementations.
pub fn os_std_op_registry() -> handl_plugin_lib_handl::OpRegistry {
    os_op_registry(
        StdFs,
        StdEnv,
        StdPath,
        StdTime,
        StdCmd,
        StdProcess::default(),
        StdTmp,
        StdSocket::default(),
        StdSig,
    )
}

// ---------------------------------------------------------------------------
// OsStdLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `os-std` implementation via
/// the generic [`HandlLibraryPlugin`] with an
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The OS interface is declared in `os.hnl`; this crate's `handl`
/// bindings are declared in `os_std.hnl`. The Rust handls (wrapping
/// stdlib implementations) are registered in the op registry. The generic
/// loader handles module registration for each OS sub-module.
pub struct OsStdLibraryPlugin {
    registry: Arc<handl_plugin_lib_handl::OpRegistry>,
    capabilities: Vec<CapabilityId>,
}

impl OsStdLibraryPlugin {
    /// Create a new `OsStdLibraryPlugin` backed by stdlib implementations.
    pub fn new() -> Self {
        let registry = os_std_op_registry();
        let capabilities = parse_cap_declarations(OS_STD_HNL)
            .into_iter()
            .map(CapabilityId::from)
            .collect();
        Self {
            registry: Arc::new(registry),
            capabilities,
        }
    }
}

impl Default for OsStdLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for OsStdLibraryPlugin {
    fn id(&self) -> &str {
        "lib.os-std"
    }

    fn name(&self) -> &str {
        "lib-os-std"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let reg = self.registry.clone();
        let fs_module = HandlModule::new("fs", OS_STD_HNL, &[]).with_ops(FS_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(fs_module))?;
        let env_module = HandlModule::new("env", OS_STD_HNL, &[]).with_ops(ENV_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(env_module))?;
        let path_module = HandlModule::new("path", OS_STD_HNL, &[]).with_ops(PATH_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(path_module))?;
        let time_module = HandlModule::new("time", OS_STD_HNL, &[]).with_ops(TIME_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(time_module))?;
        let cmd_module = HandlModule::new("cmd", OS_STD_HNL, &[]).with_ops(CMD_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(cmd_module))?;
        let process_module = HandlModule::new("process", OS_STD_HNL, &[]).with_ops(PROCESS_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(process_module))?;
        let tmp_module = HandlModule::new("tmp", OS_STD_HNL, &[]).with_ops(TMP_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(tmp_module))?;
        let socket_module = HandlModule::new("socket", OS_STD_HNL, &[]).with_ops(SOCKET_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(socket_module))?;
        let sig_module = HandlModule::new("sig", OS_STD_HNL, &[]).with_ops(SIG_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(sig_module))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Build a set of [`HandlLibraryPlugin`]s for os-std with the op registry
/// (backed by stdlib implementations).
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`OsStdLibraryPlugin`]. Returns one plugin per OS
/// sub-module.
pub fn os_std_handl_plugins() -> Vec<HandlLibraryPlugin> {
    let registry = Arc::new(os_std_op_registry());
    vec![
        HandlLibraryPlugin::new_with_caps("fs", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(FS_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("env", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(ENV_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("path", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(PATH_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("time", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(TIME_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("cmd", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(CMD_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("process", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(PROCESS_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("tmp", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(TMP_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("socket", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(SOCKET_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("sig", "lib.os-std", "lib-os-std", OS_STD_HNL, &[]).with_ops(SIG_OPS, registry.clone()),
    ]
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`OsStdLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_os_std::LibraryPlugin`.
pub use OsStdLibraryPlugin as LibraryPlugin;
