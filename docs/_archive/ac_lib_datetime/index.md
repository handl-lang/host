# handl_lib_datetime

# handl-lib-datetime

Date and time utility library for the handl runtime.

Provides functions for getting the current time, formatting/parsing
ISO 8601 datetime strings, extracting components (year, month, day, etc.),
performing arithmetic (add seconds, days, months, etc.), comparing
datetimes, and computing durations.

## Architecture

```text
handl-runtime-core          handl-lib-datetime
─────────────            ────────────────
Module trait             DateTimeModule (Copy, zero-alloc)
ModuleHost               *Function structs
                         chrono → VmError
```

All function structs are `Copy` with no internal state.

## Functions

- [`DateTimeModule`] — `datetime.from_unix`, `datetime.to_unix`,
  `datetime.from_components`, `datetime.format`,
  `datetime.parse`, `datetime.to_timezone`, `datetime.to_utc`,
  `datetime.year`, `datetime.month`, `datetime.day`,
  `datetime.hour`, `datetime.minute`, `datetime.second`,
  `datetime.weekday`, `datetime.day_of_year`, `datetime.week_of_year`,
  `datetime.quarter`, `datetime.is_leap_year`, `datetime.is_weekend`,
  `datetime.add_seconds`, `datetime.add_minutes`, `datetime.add_hours`,
  `datetime.add_days`, `datetime.add_weeks`, `datetime.add_months`,
  `datetime.add_years`, `datetime.compare`, `datetime.is_before`,
  `datetime.is_after`, `datetime.is_equal`, `datetime.is_between`,
  `datetime.is_between_inclusive`,
  `datetime.duration_between`, `datetime.duration_seconds`,
  `datetime.duration_minutes`, `datetime.duration_hours`,
  `datetime.duration_days`,
  `datetime.start_of_day`, `datetime.end_of_day`,
  `datetime.start_of_hour`, `datetime.end_of_hour`,
  `datetime.start_of_month`, `datetime.end_of_month`,
  `datetime.start_of_year`, `datetime.end_of_year`,
  `datetime.is_valid`

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_datetime::DateTimeModule;

let host = ModuleHost::new()
    .with(DateTimeModule);
```

## Modules

### [`handl_lib_datetime`](handl_lib_datetime.md)

*2 modules*

### [`datetime`](datetime.md)

*1 struct, 11 modules*

### [`datetime::arithmetic`](datetime/arithmetic.md)

*1 function, 7 structs*

### [`datetime::bounds`](datetime/bounds.md)

*1 macro, 8 structs*

### [`datetime::common`](datetime/common.md)

*13 functions*

### [`datetime::compare`](datetime/compare.md)

*6 structs*

### [`datetime::components`](datetime/components.md)

*1 macro, 12 structs*

### [`datetime::construct`](datetime/construct.md)

*1 function, 1 struct*

### [`datetime::convert`](datetime/convert.md)

*2 structs*

### [`datetime::duration`](datetime/duration.md)

*1 function, 5 structs*

### [`datetime::format_parse`](datetime/format_parse.md)

*2 structs*

### [`datetime::timezone`](datetime/timezone.md)

*2 structs*

### [`datetime::validate`](datetime/validate.md)

*1 struct*

### [`error`](error.md)

*1 enum*

