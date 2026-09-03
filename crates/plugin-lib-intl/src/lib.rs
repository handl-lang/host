//! # handl-lib-intl
//!
//! Internationalization library for the handl runtime, backed by ICU4X.
//!
//! Provides locale-aware functions for formatting numbers, selecting
//! plural categories, formatting lists, relative time, dates, durations,
//! comparing strings, display names, and text segmentation.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The intl interface is declared in
//! `kit-stdlib/data/intl/intl.hnl` via `---` doc comments and `cap`
//! declarations. The [`IntlLibraryPlugin`] wraps a generic
//! [`HandlLibraryPlugin`] for the interface declaration (id, name,
//! capabilities) while contributing the [`IntlModule`] into the
//! [`ModuleRuntime`]. All the Rust `Module`/`Function` impls are
//! preserved unchanged; only the plugin scaffolding was migrated.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-intl
//! ─────────────            ────────────
//! Module trait             IntlModule
//! ModuleRuntime               ICU4X compiled_data (600+ locales)
//!                          IntlError → VmError
//!                          *Function structs
//! ```
//!
//! All locale data is provided by ICU4X's `compiled_data` feature,
//! which bundles CLDR data at compile time. This gives full CLDR
//! coverage (600+ locales) with no runtime data loading.
//!
//! For smaller binaries, use [`IntlModule::with_blob_provider`] to
//! load a subset of CLDR data at runtime from a `.postcard` blob.
//!
//! ## Functions
//!
//! - [`IntlModule`] — `intl.format_number`, `intl.select_plural`,
//!   `intl.format_list`, `intl.format_relative_time`, `intl.format_date`,
//!   `intl.format_duration`, `intl.compare_strings`, `intl.display_name`,
//!   `intl.segment`
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_module::ModuleRuntime;
//! use handl_plugin_lib_intl::IntlModule;
//!
//! let host = ModuleRuntime::new()
//!     .with(IntlModule);
//! ```
//!
//! Expressions can then call `intl.format_number(1234567.89, "pt-BR")`,
//! `intl.select_plural(0, "ar")`, `intl.format_list(["a", "b", "c"], "en")`,
//! `intl.format_relative_time(5, "second")`, `intl.format_date("2024-05-08")`,
//! etc.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_module::{ModuleRule, ModuleRuntime};

mod common;
mod compare_strings;
mod display_name;
mod error;
mod format_date;
mod format_duration;
mod format_list;
mod format_number;
mod format_relative_time;
mod locale;
mod locale_match;
pub mod module;
mod parts;
pub mod provider;
mod segment;
mod select_plural;

pub use error::IntlError;
pub use module::IntlModule;
pub use provider::IntlProvider;

#[cfg(test)]
mod locale_match_tests;
#[cfg(test)]
mod locale_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod supported_locales_tests;

pub mod api;
pub mod extra;
pub mod icu4x;
pub mod rust;
pub mod traits;

/// The handl source declaring the intl interface — the single source
/// of truth for the intl API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const INTL_HNL: &str = include_str!("../../../kit-stdlib/data/intl/intl.hnl");

// ---------------------------------------------------------------------------
// IntlLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `intl` module into the
/// [`ModuleRuntime`] and declares the interface via a
/// [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The [`HandlLibraryPlugin`] inner is used for the interface declaration
/// — id, name, and capabilities parsed from `intl.hnl` — while the custom
/// [`Plugin::contribute`] implementation registers the [`IntlModule`].
///
/// All the Rust `Module`/`Function` impls are preserved unchanged; only the
/// plugin scaffolding was migrated to the op/handl form.
pub struct IntlLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl IntlLibraryPlugin {
    /// Create a new `IntlLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "intl",
                "lib.intl",
                "lib-intl",
                INTL_HNL,
                &[],
            ),
        }
    }
}

impl Default for IntlLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for IntlLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(IntlModule::new()))?;
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

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`IntlLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_intl::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use IntlLibraryPlugin as LibraryPlugin;
