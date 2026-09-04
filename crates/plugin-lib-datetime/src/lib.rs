//! # handl-lib-datetime
//!
//! Date and time utility library for the handl runtime.
//!
//! Provides functions for getting the current time, formatting/parsing
//! ISO 8601 datetime strings, extracting components (year, month, day, etc.),
//! performing arithmetic (add seconds, days, months, etc.), comparing
//! datetimes, and computing durations.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The datetime interface is declared in
//! `handl-packages/kit-stdlib/datetime/datetime.hnl` via `op` declarations, and the
//! Rust datetime logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`DatetimeLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry.
//!
//! The [`DateTimeModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

mod datetime;
mod error;

pub use datetime::DateTimeModule;
pub use error::DatetimeError;

/// The handl source declaring the datetime `op` interface — the single source
/// of truth for the datetime API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const DATETIME_HNL: &str = include_str!("../../../handl-packages/kit-stdlib/datetime/datetime.hnl");

/// The op names declared in `datetime.hnl`.
const DATETIME_OPS: &[&str] = &[
    "datetime.from_unix",
    "datetime.to_unix",
    "datetime.from_components",
    "datetime.format",
    "datetime.parse",
    "datetime.to_timezone",
    "datetime.to_utc",
    "datetime.year",
    "datetime.month",
    "datetime.day",
    "datetime.hour",
    "datetime.minute",
    "datetime.second",
    "datetime.weekday",
    "datetime.day_of_year",
    "datetime.week_of_year",
    "datetime.quarter",
    "datetime.is_leap_year",
    "datetime.is_weekend",
    "datetime.add_seconds",
    "datetime.add_minutes",
    "datetime.add_hours",
    "datetime.add_days",
    "datetime.add_weeks",
    "datetime.add_months",
    "datetime.add_years",
    "datetime.compare",
    "datetime.is_before",
    "datetime.is_after",
    "datetime.is_equal",
    "datetime.is_between",
    "datetime.is_between_inclusive",
    "datetime.duration_between",
    "datetime.duration_seconds",
    "datetime.duration_minutes",
    "datetime.duration_hours",
    "datetime.duration_days",
    "datetime.start_of_day",
    "datetime.end_of_day",
    "datetime.start_of_hour",
    "datetime.end_of_hour",
    "datetime.start_of_month",
    "datetime.end_of_month",
    "datetime.start_of_year",
    "datetime.end_of_year",
    "datetime.is_valid",
];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for datetime, registering the Rust handls under
/// their op names.
pub fn datetime_op_registry() -> OpRegistry {
    use datetime::*;
    let mut registry = OpRegistry::new();
    registry.register("datetime.from_unix", Arc::new(FromUnix));
    registry.register("datetime.to_unix", Arc::new(ToUnix));
    registry.register("datetime.from_components", Arc::new(FromComponents));
    registry.register("datetime.format", Arc::new(Format));
    registry.register("datetime.parse", Arc::new(Parse));
    registry.register("datetime.to_timezone", Arc::new(ToTimezone));
    registry.register("datetime.to_utc", Arc::new(ToUtc));
    registry.register("datetime.year", Arc::new(Year));
    registry.register("datetime.month", Arc::new(Month));
    registry.register("datetime.day", Arc::new(Day));
    registry.register("datetime.hour", Arc::new(Hour));
    registry.register("datetime.minute", Arc::new(Minute));
    registry.register("datetime.second", Arc::new(Second));
    registry.register("datetime.weekday", Arc::new(Weekday));
    registry.register("datetime.day_of_year", Arc::new(DayOfYear));
    registry.register("datetime.week_of_year", Arc::new(WeekOfYear));
    registry.register("datetime.quarter", Arc::new(Quarter));
    registry.register("datetime.is_leap_year", Arc::new(IsLeapYear));
    registry.register("datetime.is_weekend", Arc::new(IsWeekend));
    registry.register("datetime.add_seconds", Arc::new(AddSeconds));
    registry.register("datetime.add_minutes", Arc::new(AddMinutes));
    registry.register("datetime.add_hours", Arc::new(AddHours));
    registry.register("datetime.add_days", Arc::new(AddDays));
    registry.register("datetime.add_weeks", Arc::new(AddWeeks));
    registry.register("datetime.add_months", Arc::new(AddMonths));
    registry.register("datetime.add_years", Arc::new(AddYears));
    registry.register("datetime.compare", Arc::new(Compare));
    registry.register("datetime.is_before", Arc::new(IsBefore));
    registry.register("datetime.is_after", Arc::new(IsAfter));
    registry.register("datetime.is_equal", Arc::new(IsEqual));
    registry.register("datetime.is_between", Arc::new(IsBetween));
    registry.register("datetime.is_between_inclusive", Arc::new(IsBetweenInclusive));
    registry.register("datetime.duration_between", Arc::new(DurationBetween));
    registry.register("datetime.duration_seconds", Arc::new(DurationSeconds));
    registry.register("datetime.duration_minutes", Arc::new(DurationMinutes));
    registry.register("datetime.duration_hours", Arc::new(DurationHours));
    registry.register("datetime.duration_days", Arc::new(DurationDays));
    registry.register("datetime.start_of_day", Arc::new(StartOfDay));
    registry.register("datetime.end_of_day", Arc::new(EndOfDay));
    registry.register("datetime.start_of_hour", Arc::new(StartOfHour));
    registry.register("datetime.end_of_hour", Arc::new(EndOfHour));
    registry.register("datetime.start_of_month", Arc::new(StartOfMonth));
    registry.register("datetime.end_of_month", Arc::new(EndOfMonth));
    registry.register("datetime.start_of_year", Arc::new(StartOfYear));
    registry.register("datetime.end_of_year", Arc::new(EndOfYear));
    registry.register("datetime.is_valid", Arc::new(IsValid));
    registry
}

// ---------------------------------------------------------------------------
// DatetimeLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `datetime` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct DatetimeLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl DatetimeLibraryPlugin {
    /// Create a new `DatetimeLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(datetime_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "datetime",
                "lib.datetime",
                "lib-datetime",
                DATETIME_HNL,
                &[],
            )
            .with_ops(DATETIME_OPS, registry),
        }
    }
}

impl Default for DatetimeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for DatetimeLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for datetime with the op registry.
pub fn datetime_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(datetime_op_registry());
    HandlLibraryPlugin::new_with_caps("datetime", "lib.datetime", "lib-datetime", DATETIME_HNL, &[])
        .with_ops(DATETIME_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`DatetimeLibraryPlugin`].
pub use DatetimeLibraryPlugin as LibraryPlugin;
