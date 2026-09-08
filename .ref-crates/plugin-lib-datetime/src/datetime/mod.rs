mod arithmetic;
mod bounds;
mod common;
mod compare;
mod components;
mod construct;
mod convert;
mod duration;
mod format_parse;
mod timezone;
mod validate;

pub use arithmetic::{AddDays, AddHours, AddMinutes, AddMonths, AddSeconds, AddWeeks, AddYears};
pub use bounds::{
    EndOfDay, EndOfHour, EndOfMonth, EndOfYear, StartOfDay, StartOfHour, StartOfMonth, StartOfYear,
};
pub use compare::{Compare, IsAfter, IsBefore, IsBetween, IsBetweenInclusive, IsEqual};
pub use components::{
    Day, DayOfYear, Hour, IsLeapYear, IsWeekend, Minute, Month, Quarter, Second, WeekOfYear,
    Weekday, Year,
};
pub use construct::FromComponents;
pub use convert::{FromUnix, ToUnix};
pub use duration::{
    DurationBetween, DurationDays, DurationHours, DurationMinutes, DurationSeconds,
};
pub use format_parse::{Format, Parse};
pub use timezone::{ToTimezone, ToUtc};
pub use validate::IsValid;

use std::sync::{Arc, OnceLock};

use handl_platform::Function;
use handl_platform::Module;
use handl_plugin_lib_handl::HandlModule;

/// The handl source declaring the datetime `op` interface — the single
/// source of truth for the datetime API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const DATETIME_HNL: &str = include_str!("../../../../handl-packages/datetime/datetime.hnl");

/// The op names declared in `datetime.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`] via [`crate::datetime_op_registry`].
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

/// Build the [`HandlModule`] for datetime.
///
/// All 46 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the [`HandlModule`] dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_datetime_module() -> HandlModule {
    let registry = Arc::new(crate::datetime_op_registry());
    HandlModule::new("datetime", DATETIME_HNL, &[]).with_ops(DATETIME_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for datetime.
static DATETIME_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for datetime, initializing it on first call.
fn datetime_module() -> &'static HandlModule {
    DATETIME_MODULE.get_or_init(build_datetime_module)
}

/// The `datetime` module — date and time utility functions.
///
/// Provides functions callable as `datetime.from_unix(ts)`, `datetime.format(d, fmt)`,
/// `datetime.parse(s)`, `datetime.add_days(d, n)`, etc. in handl expressions.
///
/// Datetimes are represented as ISO 8601 strings (`YYYY-MM-DDTHH:MM:SSZ`).
/// Unix timestamps are represented as numbers (seconds since epoch).
///
/// The interface is declared in `datetime.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct only delegates
/// [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct DateTimeModule;

impl Module for DateTimeModule {
    fn name(&self) -> &str {
        "datetime"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        datetime_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        datetime_module().functions()
    }
}

// Test modules temporarily removed while datetime API is stabilised.
