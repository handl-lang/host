//! Plugin registration for the `tag` library (ADR-0055).

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::TagModule;

/// A `Library` plugin that registers the `tag` tagged-values capability
/// (ADR-0055).
///
/// Registering this plugin offers the `tag.tag` minting capability (and the
/// `tag.match_tag` / `tag.untag` / `tag.tag_of` / `tag.is_tag` inspection
/// ops) to handl programs. A host that does not register this plugin does
/// not offer tagging — user code cannot mint tagged values without it,
/// which is the capability boundary ADR-0055 §1 establishes.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.tag"
    }

    fn name(&self) -> &'static str {
        "lib-tag"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TagModule))?;
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
