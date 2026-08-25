**handl_lib_intl > format_relative_time**

# Module: format_relative_time

## Contents

**Structs**

- [`FormatRelativeTime`](#formatrelativetime) - `intl.format_relative_time(value: number, unit: string, locale?: string, opts?: object) → string`

**Functions**

- [`create_formatter`](#create_formatter)

---

## handl_lib_intl::format_relative_time::FormatRelativeTime

*Struct*

`intl.format_relative_time(value: number, unit: string, locale?: string, opts?: object) → string`

Formats a relative time using ICU4X.

`unit` must be one of: `"second"`, `"minute"`, `"hour"`, `"day"`,
`"week"`, `"month"`, `"quarter"`, `"year"`.

Options:
- `style`: `"long"` (default), `"short"`, or `"narrow"`
- `numeric`: `"always"` (default) or `"auto"` (e.g. "yesterday" instead of "-1 days")

Examples:
- `intl.format_relative_time(5, "second")` → `"in 5 seconds"`
- `intl.format_relative_time(-10, "second")` → `"10 seconds ago"`
- `intl.format_relative_time(2, "day", "es", {numeric: "auto"})` → `"pasado mañana"`
- `intl.format_relative_time(3, "year", "bn")` → `"৩ বছরে"`

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
  - `fn clone(self: &Self) -> FormatRelativeTime`



## handl_lib_intl::format_relative_time::create_formatter

*Function*

```rust
fn create_formatter(locale: &icu::locale::Locale, unit: &str, style: &str, options: icu::experimental::relativetime::RelativeTimeFormatterOptions, dp: &super::common::DataProvider) -> Result<icu::experimental::relativetime::RelativeTimeFormatter, ac_runtime_values::Error>
```



