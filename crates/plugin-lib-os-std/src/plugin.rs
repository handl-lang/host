//! Plugin registration for the `os-std` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_os::{
    CmdModule, EnvModule, FsModule, PathModule, ProcessModule, SigModule, SocketModule,
    TimeModule, TmpModule,
};
use handl_plugin_module::{ModuleRuntime, ModuleRule};

use crate::{StdCmd, StdEnv, StdFs, StdPath, StdProcess, StdSig, StdSocket, StdTime, StdTmp};

/// A `Library` plugin that registers the `os-std` implementation.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.os-std"
    }

    fn name(&self) -> &'static str {
        "lib-os-std"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(FsModule::new(StdFs)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(EnvModule::new(StdEnv)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(PathModule::new(StdPath)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TimeModule::new(StdTime)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(CmdModule::new(StdCmd)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(ProcessModule::new(
            StdProcess::default(),
        )))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TmpModule::new(StdTmp)))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(SocketModule::new(
            StdSocket::default(),
        )))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(SigModule::new(StdSig)))?;
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
