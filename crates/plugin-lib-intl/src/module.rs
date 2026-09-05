use std::sync::Arc;

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use super::compare_strings::CompareStrings;
use super::display_name::DisplayName;
use super::format_date::{FormatDate, FormatDateToParts};
use super::format_duration::FormatDuration;
use super::format_list::FormatList;
use super::format_number::{FormatNumber, FormatNumberToParts};
use super::format_relative_time::FormatRelativeTime;
use super::locale::LocaleInfo;
use super::locale_match::{GetCanonicalLocales, SupportedLocalesOf};
use super::provider::IntlProvider;
use super::segment::Segment;
use super::select_plural::SelectPlural;

/// The handl source declaring the intl `op` interface — the single source
/// of truth for the intl API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const INTL_HNL: &str = include_str!("../../../handl-packages/intl/intl.hnl");

/// The op names declared in `intl.hnl`.
const INTL_OPS: &[&str] = &[
    "intl.format_number",
    "intl.format_number_to_parts",
    "intl.select_plural",
    "intl.format_list",
    "intl.format_relative_time",
    "intl.format_date",
    "intl.format_date_to_parts",
    "intl.format_duration",
    "intl.compare_strings",
    "intl.display_name",
    "intl.segment",
    "intl.locale",
    "intl.get_canonical_locales",
    "intl.supported_locales_of",
];

/// Build an [`OpRegistry`] for intl using ICU4X compiled CLDR data.
fn intl_op_registry_default() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("intl.format_number", Arc::new(FormatNumber::new()));
    registry.register(
        "intl.format_number_to_parts",
        Arc::new(FormatNumberToParts::new()),
    );
    registry.register("intl.select_plural", Arc::new(SelectPlural::new()));
    registry.register("intl.format_list", Arc::new(FormatList::new()));
    registry.register(
        "intl.format_relative_time",
        Arc::new(FormatRelativeTime::new()),
    );
    registry.register("intl.format_date", Arc::new(FormatDate::new()));
    registry.register(
        "intl.format_date_to_parts",
        Arc::new(FormatDateToParts::new()),
    );
    registry.register("intl.format_duration", Arc::new(FormatDuration::new()));
    registry.register("intl.compare_strings", Arc::new(CompareStrings::new()));
    registry.register("intl.display_name", Arc::new(DisplayName::new()));
    registry.register("intl.segment", Arc::new(Segment::new()));
    registry.register(
        "intl.supported_locales_of",
        Arc::new(SupportedLocalesOf::new()),
    );
    registry.register("intl.get_canonical_locales", Arc::new(GetCanonicalLocales));
    registry.register("intl.locale", Arc::new(LocaleInfo));
    registry
}

/// Build an [`OpRegistry`] for intl using a runtime-loaded data blob.
fn intl_op_registry_with_provider(provider: &IntlProvider) -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register(
        "intl.format_number",
        Arc::new(FormatNumber::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_number_to_parts",
        Arc::new(FormatNumberToParts::with_provider(provider.clone())),
    );
    registry.register(
        "intl.select_plural",
        Arc::new(SelectPlural::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_list",
        Arc::new(FormatList::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_relative_time",
        Arc::new(FormatRelativeTime::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_date",
        Arc::new(FormatDate::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_date_to_parts",
        Arc::new(FormatDateToParts::with_provider(provider.clone())),
    );
    registry.register(
        "intl.format_duration",
        Arc::new(FormatDuration::with_provider(provider.clone())),
    );
    registry.register(
        "intl.compare_strings",
        Arc::new(CompareStrings::with_provider(provider.clone())),
    );
    registry.register(
        "intl.display_name",
        Arc::new(DisplayName::with_provider(provider.clone())),
    );
    registry.register(
        "intl.segment",
        Arc::new(Segment::with_provider(provider.clone())),
    );
    registry.register(
        "intl.supported_locales_of",
        Arc::new(SupportedLocalesOf::with_provider(provider.clone())),
    );
    registry.register("intl.get_canonical_locales", Arc::new(GetCanonicalLocales));
    registry.register("intl.locale", Arc::new(LocaleInfo));
    registry
}

/// The `intl` module — internationalization functions.
///
/// Provides functions callable as `intl.format_number(n, locale?, opts?)`,
/// `intl.select_plural(n, locale?, opts?)`, etc. in handl expressions.
///
/// By default, uses ICU4X compiled CLDR data (600+ locales).
/// For smaller binaries, construct with [`IntlModule::with_provider`]
/// to use a runtime-loaded data blob.
pub struct IntlModule {
    provider: Option<IntlProvider>,
    module: HandlModule,
}

impl Default for IntlModule {
    fn default() -> Self {
        Self::new()
    }
}

impl IntlModule {
    /// Create a new `IntlModule` using ICU4X compiled CLDR data.
    pub fn new() -> Self {
        let registry = intl_op_registry_default();
        Self {
            provider: None,
            module: HandlModule::new("intl", INTL_HNL, &[]).with_ops(INTL_OPS, Arc::new(registry)),
        }
    }

    /// Create a new `IntlModule` using a runtime-loaded data blob.
    pub fn with_provider(provider: IntlProvider) -> Self {
        let registry = intl_op_registry_with_provider(&provider);
        Self {
            provider: Some(provider),
            module: HandlModule::new("intl", INTL_HNL, &[]).with_ops(INTL_OPS, Arc::new(registry)),
        }
    }

    /// Create a new `IntlModule` from a blob of ICU4X postcard data.
    pub fn with_blob_provider(blob: Vec<u8>) -> Self {
        Self::with_provider(IntlProvider::from_blob(blob))
    }

    /// Returns the data provider, if one was set.
    pub fn provider(&self) -> Option<&IntlProvider> {
        self.provider.as_ref()
    }
}

impl Module for IntlModule {
    fn name(&self) -> &str {
        self.module.name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
    }
}
