use handl_platform::{ContributionContext, PlatformError};

/// Seam between the bootstrap and concrete applier plugins (ADR-0036).
///
/// The bootstrap's only job is deciding *which* applier to start with; the
/// applier decides *how* host configuration is applied. Implementations of
/// this trait live in `plugin-applier-*` crates and are looked up by plugin
/// id (e.g. `applier.default`) at bootstrap time.
pub trait ApplierFactory: Send + Sync {
    /// The plugin id of the applier this factory produces.
    fn id(&self) -> &str;

    /// Register the applier plugin for a config source and fallback kit.
    fn register(
        &self,
        ctx: &mut ContributionContext,
        config_path: &str,
        fallback_kit: &str,
    ) -> Result<(), PlatformError>;
}
