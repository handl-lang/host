**handl_lib_intl > format_duration**

# Module: format_duration

## Contents

**Structs**

- [`FormatDuration`](#formatduration) - `intl.format_duration(duration: object, locale?: string, opts?: object) → string`

**Functions**

- [`apply_per_unit_options`](#apply_per_unit_options)
- [`build_duration`](#build_duration)
- [`parse_field_display`](#parse_field_display)

**Constants**

- [`DURATION_FORMATTERS`](#duration_formatters)

---

## handl_lib_intl::format_duration::DURATION_FORMATTERS

*Constant*: `$crate::thread::LocalKey<std::cell::RefCell<std::collections::HashMap<(String, String), std::rc::Rc<icu::experimental::duration::DurationFormatter>>>>`



## handl_lib_intl::format_duration::FormatDuration

*Struct*

`intl.format_duration(duration: object, locale?: string, opts?: object) → string`

Formats a duration using ICU4X.

`duration` is an object with any of: `years`, `months`, `weeks`, `days`,
`hours`, `minutes`, `seconds`, `milliseconds`, `microseconds`, `nanoseconds`.

Options:
- `style`: `"long"` (default), `"short"`, `"narrow"`, `"digital"`
- Per-unit style: `yearsStyle`, `monthsStyle`, `weeksStyle`, `daysStyle`,
  `hoursStyle`, `minutesStyle`, `secondsStyle`, `millisecondsStyle`,
  `microsecondsStyle`, `nanosecondsStyle` — each `"long"`, `"short"`,
  `"narrow"`, and for time units also `"numeric"` / `"2-digit"`
- Per-unit display: `yearsDisplay`, `monthsDisplay`, etc. — `"auto"` (default) or `"always"`
- `fractionalDigits`: number of fractional digits for sub-second units

Examples:
- `intl.format_duration({hours: 1, minutes: 30})` → `"1 hour, 30 minutes"`
- `intl.format_duration({seconds: 90}, "es")` → `"90 segundos"`
- `intl.format_duration({hours: 2, minutes: 5}, "en", {style: "short"})` → `"2 hr, 5 min"`
- `intl.format_duration({hours: 2}, "en", {hoursStyle: "long"})` → `"2 hours"`

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
  - `fn clone(self: &Self) -> FormatDuration`



## handl_lib_intl::format_duration::apply_per_unit_options

*Function*

```rust
fn apply_per_unit_options(options: & mut icu::experimental::duration::options::DurationFormatterOptions, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::format_duration::build_duration

*Function*

```rust
fn build_duration(obj: &serde_json::Map<String, serde_json::Value>) -> icu::experimental::duration::Duration
```



## handl_lib_intl::format_duration::parse_field_display

*Function*

```rust
fn parse_field_display(opts: Option<&serde_json::Map<String, serde_json::Value>>, key: &str) -> Option<icu::experimental::duration::options::FieldDisplay>
```



