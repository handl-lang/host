//! Plugin registration for the `utils` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_vm_common::{ModuleRuntime, ModuleRule};

use crate::{
    Base64Module, BoolModule, CmpModule, EncodingModule, FnModule, HexModule, JsonModule,
    MathModule, MoodModule, OptionModule, ParseModule, ResultModule, SerializeModule, StrModule,
    TomlModule, TypeModule, UrlRfcModule, UrlWhatwgModule, YamlModule,
};

/// A `Library` plugin that registers the `utils` implementation.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.utils"
    }

    fn name(&self) -> &'static str {
        "lib-utils"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(MathModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(StrModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(BoolModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(CmpModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(FnModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TypeModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(OptionModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(ResultModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(MoodModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(EncodingModule::new()))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(ParseModule::new()))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(SerializeModule::new()))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(Base64Module))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(HexModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(JsonModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TomlModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(YamlModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(UrlRfcModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(UrlWhatwgModule))?;
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
