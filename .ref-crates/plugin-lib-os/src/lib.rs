//! # handl-lib-os
//!
//! Abstract OS service traits for the handl runtime.
//!
//! Defines abstract service interfaces that bridge host capabilities
//! (filesystem, environment, path manipulation, time, command execution, temp files,
//! sockets, signals)
//! into callable [`Module`] implementations.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The OS interface is declared in
//! `handl-packages/os/os.hnl` via `op` declarations, and the Rust handls
//! (handlers) that delegate to the service traits are registered in an
//! [`OpRegistry`] via [`os_op_registry`]. The [`OsLibraryPlugin`]
//! wraps the generic [`HandlLibraryPlugin`] with the op registry (backed by
//! stubs by default), eliminating the per-library `Plugin` trait scaffolding.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-os                     handl-lib-os-std
//! ─────────────            ──────────────────            ─────────────────────
//! Module trait             Fs trait                      StdFs (std::fs)
//! ModuleRuntime               Env trait                     StdEnv (std::env)
//!                          Path trait                    StdPath (std::path)
//!                          Time trait                    StdTime (std::time)
//!                          Cmd trait                     StdCmd (std::process)
//!                          Tmp trait                     StdTmp (std::temp_dir)
//!                          Socket trait                  StdSocket (std::net)
//!                          Sig trait                     StdSig (std::process)
//!                          OsError
//!                          *Module adapters
//!                          OsLibraryPlugin              OsStdLibraryPlugin
//! ```
//!
//! - **This crate** — abstract service traits + module adapters + test stubs.
//!   No I/O, no platform dependencies.
//! - **`handl-lib-os-std`** — concrete implementations using
//!   Rust stdlib.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_module::ModuleRuntime;
//! use handl_plugin_lib_utils::{MathModule, TypeModule};
//! use handl_plugin_lib_os::{
//!     FsModule, EnvModule, PathModule, TimeModule,
//!     CmdModule, TmpModule, SocketModule, SigModule,
//!     fs::fs_stub::StubFs,
//!     env::env_stub::StubEnv,
//!     path::path_stub::StubPath,
//!     time::time_stub::StubTime,
//!     cmd::cmd_stub::StubCmd,
//!     tmp::tmp_stub::StubTmp,
//!     socket::socket_stub::StubSocket,
//!     sig::sig_stub::StubSig,
//! };
//!
//! let host = ModuleRuntime::new()
//!     .with(MathModule)
//!     .with(TypeModule)
//!     .with(FsModule::new(StubFs::default()))
//!     .with(EnvModule::new(StubEnv::default()))
//!     .with(PathModule::new(StubPath::default()))
//!     .with(TimeModule::new(StubTime::default()))
//!     .with(CmdModule::new(StubCmd::default()))
//!     .with(TmpModule::new(StubTmp::default()))
//!     .with(SocketModule::new(StubSocket::default()))
//!     .with(SigModule::new(StubSig::default()));
//! ```
//!
//! Expressions can then call `fs.read("config.yml")`, `env.get("HOME")`,
//! `path.join("a", "b")`, `time.now()`, `cmd.run("echo", "hello")`, `tmp.file()`,
//! `socket.connect("addr")`, `sig.send(42, "SIGINT")`, etc.

#![allow(clippy::module_inception)]

mod cmd;
mod env;
mod error;
mod fs;
mod path;
mod process;
mod sig;
mod socket;
mod time;
mod tmp;

use std::sync::Arc;

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::{HandlLibraryPlugin, HandlModule, OpRegistry, parse_cap_declarations};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

pub use error::OsError;

pub use fs::{FS_OPS, Fs, FsModule, register_fs_ops};

pub use env::{ENV_OPS, Env, EnvModule, register_env_ops};

pub use path::{PATH_OPS, Path, PathModule, register_path_ops};

pub use process::{PROCESS_OPS, Process, ProcessModule, ProcessStatus, register_process_ops};

pub use time::{TIME_OPS, Time, TimeModule, register_time_ops};

pub use cmd::{CMD_OPS, Cmd, CmdModule, CmdOutput, register_cmd_ops};

pub use tmp::{TMP_OPS, Tmp, TmpModule, register_tmp_ops};

pub use socket::{
    AsyncSocket, AsyncSocketModule, SOCKET_OPS, Socket, SocketModule, register_socket_ops,
};

pub use sig::{SIG_OPS, Sig, SigModule, register_sig_ops};

// Re-export stubs for convenience.
pub use cmd::cmd_stub::StubCmd;
pub use env::env_stub::StubEnv;
pub use fs::fs_stub::StubFs;
pub use path::path_stub::StubPath;
pub use process::process_stub::StubProcess;
pub use sig::sig_stub::StubSig;
pub use socket::socket_stub::StubSocket;
pub use time::time_stub::StubTime;
pub use tmp::tmp_stub::StubTmp;

/// The handl source declaring the os `op` interface — the single source
/// of truth for the OS API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const OS_HNL: &str = include_str!("../../../handl-packages/os/os.hnl");

/// The op names declared in `os.hnl` — the complete OS interface surface.
pub const OS_OPS: &[&str] = &[
    "fs.read",
    "fs.write",
    "fs.exists",
    "fs.remove",
    "fs.list",
    "env.get",
    "env.has",
    "env.keys",
    "path.join",
    "path.resolve",
    "path.basename",
    "path.dirname",
    "path.extname",
    "time.now",
    "time.epoch",
    "cmd.run",
    "process.spawn",
    "process.kill",
    "process.wait",
    "process.status",
    "tmp.file",
    "tmp.dir",
    "socket.connect",
    "socket.send",
    "socket.recv",
    "socket.close",
    "sig.send",
    "sig.list",
];

/// Build an [`OpRegistry`] for the OS ops, registering Rust handls that
/// delegate to the given service implementations under their op names.
///
/// This is the shared registry builder used by both the interface crate
/// (with stubs for testing) and the `handl-lib-os-std` impl crate
/// (with std-backed implementations). Each op name (e.g. `"fs.read"`)
/// maps to a `Function` struct that parses its JSON arguments and
/// dispatches to the appropriate service trait method.
pub fn os_op_registry(
    fs: impl Fs + 'static,
    env: impl Env + 'static,
    path: impl Path + 'static,
    time: impl Time + 'static,
    cmd: impl Cmd + 'static,
    process: impl Process + 'static,
    tmp: impl Tmp + 'static,
    socket: impl Socket + 'static,
    sig: impl Sig + 'static,
) -> OpRegistry {
    let mut registry = OpRegistry::new();
    register_fs_ops(&mut registry, fs);
    register_env_ops(&mut registry, env);
    register_path_ops(&mut registry, path);
    register_time_ops(&mut registry, time);
    register_cmd_ops(&mut registry, cmd);
    register_process_ops(&mut registry, process);
    register_tmp_ops(&mut registry, tmp);
    register_socket_ops(&mut registry, socket);
    register_sig_ops(&mut registry, sig);
    registry
}

// ---------------------------------------------------------------------------
// OsLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `os` interface via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The OS interface is declared in `os.hnl` via `op`/`handl` declarations;
/// the Rust handls (delegating to stub implementations by default) are
/// registered in the op registry. The generic loader handles module
/// registration for each OS sub-module (fs, env, path, time, cmd, process,
/// tmp, socket, sig).
///
/// Concrete backends such as `handl-lib-os-std` provide their own
/// plugin ([`OsStdLibraryPlugin`]) with real service implementations.
pub struct OsLibraryPlugin {
    registry: Arc<OpRegistry>,
    capabilities: Vec<CapabilityId>,
}

impl OsLibraryPlugin {
    /// Create a new `OsLibraryPlugin` backed by stub implementations.
    pub fn new() -> Self {
        let registry = os_op_registry(
            StubFs::default(),
            StubEnv::default(),
            StubPath::default(),
            StubTime::default(),
            StubCmd::default(),
            StubProcess::default(),
            StubTmp::default(),
            StubSocket::default(),
            StubSig::default(),
        );
        let capabilities = parse_cap_declarations(OS_HNL)
            .into_iter()
            .map(CapabilityId::from)
            .collect();
        Self {
            registry: Arc::new(registry),
            capabilities,
        }
    }
}

impl Default for OsLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for OsLibraryPlugin {
    fn id(&self) -> &str {
        "lib.os"
    }

    fn name(&self) -> &str {
        "lib-os"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let reg = self.registry.clone();
        // fs module
        let fs_module = HandlModule::new("fs", OS_HNL, &[]).with_ops(FS_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(fs_module))?;
        // env module
        let env_module = HandlModule::new("env", OS_HNL, &[]).with_ops(ENV_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(env_module))?;
        // path module
        let path_module = HandlModule::new("path", OS_HNL, &[]).with_ops(PATH_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(path_module))?;
        // time module
        let time_module = HandlModule::new("time", OS_HNL, &[]).with_ops(TIME_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(time_module))?;
        // cmd module
        let cmd_module = HandlModule::new("cmd", OS_HNL, &[]).with_ops(CMD_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(cmd_module))?;
        // process module
        let process_module =
            HandlModule::new("process", OS_HNL, &[]).with_ops(PROCESS_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(process_module))?;
        // tmp module
        let tmp_module = HandlModule::new("tmp", OS_HNL, &[]).with_ops(TMP_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(tmp_module))?;
        // socket module
        let socket_module =
            HandlModule::new("socket", OS_HNL, &[]).with_ops(SOCKET_OPS, reg.clone());
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(socket_module))?;
        // sig module
        let sig_module = HandlModule::new("sig", OS_HNL, &[]).with_ops(SIG_OPS, reg.clone());
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

/// Build a set of [`HandlLibraryPlugin`]s for os with the op registry
/// (backed by stubs).
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`OsLibraryPlugin`]. Returns one plugin per OS
/// sub-module.
pub fn os_handl_plugins() -> Vec<HandlLibraryPlugin> {
    let registry = Arc::new(os_op_registry(
        StubFs::default(),
        StubEnv::default(),
        StubPath::default(),
        StubTime::default(),
        StubCmd::default(),
        StubProcess::default(),
        StubTmp::default(),
        StubSocket::default(),
        StubSig::default(),
    ));
    vec![
        HandlLibraryPlugin::new_with_caps("fs", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(FS_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("env", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(ENV_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("path", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(PATH_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("time", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(TIME_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("cmd", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(CMD_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("process", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(PROCESS_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("tmp", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(TMP_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("socket", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(SOCKET_OPS, registry.clone()),
        HandlLibraryPlugin::new_with_caps("sig", "lib.os", "lib-os", OS_HNL, &[])
            .with_ops(SIG_OPS, registry.clone()),
    ]
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`OsLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_os::LibraryPlugin`.
pub use OsLibraryPlugin as LibraryPlugin;
