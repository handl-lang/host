//! Plugin registration for the `utils` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_vm_common::{ModuleHost, ModuleRule};

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
        ctx.contribute::<ModuleHost>(ModuleRule::new(MathModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(StrModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(BoolModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(CmpModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(FnModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(TypeModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(OptionModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(ResultModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(MoodModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(EncodingModule::new()))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(ParseModule::new()))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(SerializeModule::new()))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(Base64Module))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(HexModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(JsonModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(TomlModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(YamlModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(UrlRfcModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(UrlWhatwgModule))?;
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
