//! Plugin registration for the `stats` library.
//!
//! This is a thin wrapper that delegates to the generic
//! [`HandlLibraryPlugin`] from [`handl_plugin_lib_handl`], while keeping
//! the same public API (`LibraryPlugin`) as before.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_lib_utils::{MathModule, TypeModule};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::StatsModule;

/// The handl source defining the stats record — the single source of truth
/// for the stats logic. Loaded at compile time from the kit-stdlib data dir.
const STATS_HNL: &str = include_str!("../../../kit-stdlib/data/stats/stats.hnl");

/// The function (record field) names exposed by the stats module.
const STATS_FUNCS: &[&str] = &["sum", "avg", "product", "min", "max", "median", "stddev"];

/// A `Library` plugin that registers the `stats` implementation.
///
/// This delegates to the generic [`HandlLibraryPlugin`] for the module
/// contribution, while also registering the [`StatsModule`] (which provides
/// the same `Function` impls that delegate to the generic loader).
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.stats"
    }

    fn name(&self) -> &'static str {
        "lib-stats"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        // Register the StatsModule (which provides Function impls that
        // delegate to the generic HandlModule loader).
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(StatsModule))?;
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

/// Build a [`HandlLibraryPlugin`] for stats, with `MathModule` (for
/// `math.sqrt` in `stddev`) and `TypeModule` (for `typeof.type_of`
/// non-number filtering) as dependencies.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`LibraryPlugin`].
pub fn stats_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new("stats", "lib.stats", "lib-stats", STATS_HNL, STATS_FUNCS)
        .with_dependency(MathModule)
        .with_dependency(TypeModule)
}
