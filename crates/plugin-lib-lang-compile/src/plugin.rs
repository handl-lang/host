//! Plugin registration for the `compile` meta library (ADR-0047 §1, §3).

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

/// A `Library` plugin marker for the `compile` meta function.
///
/// The `compile` meta module holds an `Rc<dyn RuntimeFactory>` which is not
/// `Send + Sync`, so it cannot be contributed through the platform's
/// [`ModuleRuntime`](handl_plugin_module::ModuleRuntime) (whose
/// [`ModuleRule`](handl_plugin_module::ModuleRule) requires `Send + Sync`).
/// Instead, the host constructs a [`crate::CompileModule`] directly via
/// [`crate::CompileModule::new`] and wires it into the runtime as the
/// `delegate` passed to
/// [`RuntimeFactory::build`](handl_runtime::RuntimeFactory::build).
///
/// This plugin exists for API symmetry with the other `plugin-lib-*` crates.
/// Its [`LibraryPlugin::contribute`] is a no-op; it carries no state.
#[derive(Debug, Default, Clone, Copy)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.lang.compile"
    }

    fn name(&self) -> &'static str {
        "lib-lang-compile"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    /// No-op: the `compile` meta module holds an `Rc<dyn RuntimeFactory>`
    /// which is not `Send + Sync`, so it cannot be contributed through
    /// `ModuleRuntime`. The host wires the [`crate::CompileModule`] directly
    /// via `RuntimeFactory::build`'s `delegate` parameter. Construct it with
    /// [`crate::CompileModule::new`].
    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
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
