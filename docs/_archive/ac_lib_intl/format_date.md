**handl_lib_intl > format_date**

# Module: format_date

## Contents

**Structs**

- [`FormatDate`](#formatdate) - `intl.format_date(date: string, locale?: string, opts?: object) → string`
- [`FormatDateToParts`](#formatdatetoparts) - `intl.format_date_to_parts(date: string, locale?: string, opts?: object) → array`

**Functions**

- [`build_prefs`](#build_prefs)
- [`derive_length_from_fields`](#derive_length_from_fields)
- [`determine_date_fields`](#determine_date_fields)
- [`determine_time_precision`](#determine_time_precision)
- [`determine_year_style`](#determine_year_style)
- [`determine_zone_style`](#determine_zone_style)
- [`format_date_only`](#format_date_only)
- [`format_date_only_to_parts`](#format_date_only_to_parts)
- [`format_datetime`](#format_datetime)
- [`format_datetime_to_parts`](#format_datetime_to_parts)
- [`format_time_only`](#format_time_only)
- [`format_time_only_to_parts`](#format_time_only_to_parts)
- [`format_with_field_bag`](#format_with_field_bag)
- [`format_with_field_bag_to_parts`](#format_with_field_bag_to_parts)
- [`has_field_bag_options`](#has_field_bag_options)
- [`parse_date`](#parse_date)
- [`parse_datetime`](#parse_datetime)
- [`parse_datetime_flexible`](#parse_datetime_flexible)
- [`parse_length`](#parse_length)
- [`parse_nanos_from_str`](#parse_nanos_from_str)
- [`parse_subsecond_digits`](#parse_subsecond_digits)
- [`parse_time`](#parse_time)
- [`parse_time_only`](#parse_time_only)
- [`parse_time_precision`](#parse_time_precision)
- [`parse_timezone_offset`](#parse_timezone_offset)
- [`strip_fractional_seconds`](#strip_fractional_seconds)
- [`strip_timezone`](#strip_timezone)

---

## handl_lib_intl::format_date::FormatDate

*Struct*

`intl.format_date(date: string, locale?: string, opts?: object) → string`

Formats a date, time, or datetime string using ICU4X.

`date` must be an ISO 8601 string: `"YYYY-MM-DD"`, `"HH:MM:SS"`,
or `"YYYY-MM-DDTHH:MM:SS"`. Timezone offsets (e.g. `+05:30`, `Z`)
are parsed and used when `timeZoneName` is set in the options.
Fractional seconds (e.g. `.123`, `.123456`) are extracted and used
when `fractionalSecondDigits` is set.

Options:
- `style`: `"full"`, `"long"`, `"medium"` (default), or `"short"`
- `type`: `"date"` (default), `"datetime"`, or `"time"`
- `hourCycle`: `"h11"`, `"h12"`, `"h23"`, or `"h24"`
- `hour12`: boolean — when `true` uses 12-hour cycle, when `false` uses 24-hour. Ignored if `hourCycle` is set.
- `calendar`: `"gregory"` (default), `"buddhist"`, `"chinese"`, `"coptic"`, `"ethiopic"`, `"hebrew"`, `"indian"`, `"islamic"`, `"japanese"`, `"persian"`, `"roc"`
- `fractionalSecondDigits`: 1, 2, or 3 (number of fractional second digits)

ECMA-402 individual field options (when any of these are present, they
override `style` and `type`):
- `year`: `"numeric"`, `"2-digit"`, `"narrow"`, `"short"`, `"long"`
- `month`: `"numeric"`, `"2-digit"`, `"narrow"`, `"short"`, `"long"`
- `day`: `"numeric"`, `"2-digit"`
- `weekday`: `"narrow"`, `"short"`, `"long"`
- `hour`: `"numeric"`, `"2-digit"`
- `minute`: `"numeric"`, `"2-digit"`
- `second`: `"numeric"`, `"2-digit"`
- `era`: `"narrow"`, `"short"`, `"long"`
- `timeZoneName`: `"short"`, `"long"`, `"shortOffset"`, `"longOffset"`, `"shortGeneric"`, `"longGeneric"`

Examples:
- `intl.format_date("2024-05-08")` → `"May 8, 2024"`
- `intl.format_date("2024-05-08", "de")` → `"8. Mai 2024"`
- `intl.format_date("2024-05-08T14:30:00", "en", {type: "datetime"})` → `"May 8, 2024, 2:30:00 PM"`
- `intl.format_date("14:30:00", "en", {type: "time"})` → `"2:30:00 PM"`

**Fields:**
- `provider: Option<super::provider::IntlProvider>`

**Methods:**

- `fn new() -> Self`
- `fn with_provider(provider: IntlProvider) -> Self`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> FormatDate`



## handl_lib_intl::format_date::FormatDateToParts

*Struct*

`intl.format_date_to_parts(date: string, locale?: string, opts?: object) → array`

Like [`FormatDate`], but returns an array of `{ type, value }` objects
instead of a flat string. Uses ICU4X `write_to_parts` APIs to produce
structured part annotations (e.g. `"year"`, `"month"`, `"day"`,
`"hour"`, `"minute"`, `"second"`, `"literal"`).

**Fields:**
- `provider: Option<super::provider::IntlProvider>`

**Methods:**

- `fn new() -> Self`
- `fn with_provider(provider: IntlProvider) -> Self`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> FormatDateToParts`



## handl_lib_intl::format_date::build_prefs

*Function*

```rust
fn build_prefs(locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>) -> icu::datetime::DateTimeFormatterPreferences
```



## handl_lib_intl::format_date::derive_length_from_fields

*Function*

```rust
fn derive_length_from_fields(opts: Option<&serde_json::Map<String, serde_json::Value>>, style: &str) -> icu::datetime::options::Length
```



## handl_lib_intl::format_date::determine_date_fields

*Function*

```rust
fn determine_date_fields(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::fieldsets::builder::DateFields>
```



## handl_lib_intl::format_date::determine_time_precision

*Function*

```rust
fn determine_time_precision(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::options::TimePrecision>
```



## handl_lib_intl::format_date::determine_year_style

*Function*

```rust
fn determine_year_style(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::options::YearStyle>
```



## handl_lib_intl::format_date::determine_zone_style

*Function*

```rust
fn determine_zone_style(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::fieldsets::builder::ZoneStyle>
```



## handl_lib_intl::format_date::format_date_only

*Function*

```rust
fn format_date_only(date_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_date_only_to_parts

*Function*

```rust
fn format_date_only_to_parts(date_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_datetime

*Function*

```rust
fn format_datetime(date_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_datetime_to_parts

*Function*

```rust
fn format_datetime_to_parts(date_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_time_only

*Function*

```rust
fn format_time_only(time_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_time_only_to_parts

*Function*

```rust
fn format_time_only_to_parts(time_str: &str, locale: &icu::locale::Locale, style: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_with_field_bag

*Function*

```rust
fn format_with_field_bag(date_str: &str, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_date::format_with_field_bag_to_parts

*Function*

```rust
fn format_with_field_bag_to_parts(date_str: &str, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::has_field_bag_options

*Function*

```rust
fn has_field_bag_options(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> bool
```



## handl_lib_intl::format_date::parse_date

*Function*

```rust
fn parse_date(s: &str) -> Result<(i32, u8, u8), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::parse_datetime

*Function*

```rust
fn parse_datetime(s: &str) -> Result<(i32, u8, u8, u8, u8, u8), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::parse_datetime_flexible

*Function*

```rust
fn parse_datetime_flexible(s: &str) -> Result<(i32, u8, u8, u8, u8, u8), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::parse_length

*Function*

```rust
fn parse_length(style: &str) -> icu::datetime::options::Length
```



## handl_lib_intl::format_date::parse_nanos_from_str

*Function*

```rust
fn parse_nanos_from_str(s: &str) -> u32
```



## handl_lib_intl::format_date::parse_subsecond_digits

*Function*

```rust
fn parse_subsecond_digits(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::options::SubsecondDigits>
```



## handl_lib_intl::format_date::parse_time

*Function*

```rust
fn parse_time(time_part: &str) -> Result<(u8, u8, u8), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::parse_time_only

*Function*

```rust
fn parse_time_only(s: &str) -> Result<(u8, u8, u8), ac_runtime_values::Error>
```



## handl_lib_intl::format_date::parse_time_precision

*Function*

```rust
fn parse_time_precision(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<icu::datetime::options::TimePrecision>
```



## handl_lib_intl::format_date::parse_timezone_offset

*Function*

```rust
fn parse_timezone_offset(s: &str) -> Option<icu::time::zone::UtcOffset>
```



## handl_lib_intl::format_date::strip_fractional_seconds

*Function*

```rust
fn strip_fractional_seconds(s: &str) -> &str
```



## handl_lib_intl::format_date::strip_timezone

*Function*

```rust
fn strip_timezone(s: &str) -> &str
```



