**handl_lib_intl > format_number**

# Module: format_number

## Contents

**Structs**

- [`FormatNumber`](#formatnumber) - `intl.format_number(n: number, locale?: string, opts?: object) → string`
- [`FormatNumberToParts`](#formatnumbertoparts) - `intl.format_number_to_parts(n: number, locale?: string, opts?: object) → array`

**Functions**

- [`build_formatter_options`](#build_formatter_options)
- [`decimal_formatter`](#decimal_formatter)
- [`format_compact`](#format_compact)
- [`format_compact_to_parts`](#format_compact_to_parts)
- [`format_currency`](#format_currency)
- [`format_currency_to_parts`](#format_currency_to_parts)
- [`format_decimal_to_parts`](#format_decimal_to_parts)
- [`format_decimal_with_options`](#format_decimal_with_options)
- [`format_engineering`](#format_engineering)
- [`format_engineering_to_parts`](#format_engineering_to_parts)
- [`format_exponent`](#format_exponent)
- [`format_exponent_to_parts`](#format_exponent_to_parts)
- [`format_percent`](#format_percent)
- [`format_percent_to_parts`](#format_percent_to_parts)
- [`format_scientific`](#format_scientific)
- [`format_scientific_to_parts`](#format_scientific_to_parts)
- [`format_standard`](#format_standard)
- [`format_standard_to_parts`](#format_standard_to_parts)
- [`format_unit`](#format_unit) - Format a number with a unit (e.g. "5 meters", "30 km/h").
- [`format_unit_to_parts`](#format_unit_to_parts)

**Constants**

- [`DECIMAL_FORMATTERS`](#decimal_formatters)

---

## handl_lib_intl::format_number::DECIMAL_FORMATTERS

*Constant*: `$crate::thread::LocalKey<std::cell::RefCell<std::collections::HashMap<(usize, String, String), std::rc::Rc<icu::decimal::DecimalFormatter>>>>`



## handl_lib_intl::format_number::FormatNumber

*Struct*

`intl.format_number(n: number, locale?: string, opts?: object) → string`

Formats a number using locale-aware decimal and thousands separators
via ICU4X.

Options:
- `style`: `"decimal"` (default), `"percent"`, `"currency"`, or `"unit"`
- `currency`: 3-letter ISO 4217 code (required when `style: "currency"`)
- `currencyDisplay`: `"symbol"` (default), `"narrowSymbol"`, `"code"`, or `"name"`
- `currencySign`: `"standard"` (default) or `"accounting"` (for negative values, uses parentheses)
- `notation`: `"standard"` (default), `"compact"`, `"scientific"`, or `"engineering"`
- `compactDisplay`: `"short"` (default) or `"long"` (for compact notation)
- `minimumIntegerDigits`: minimum integer digits to display (pads with leading zeros)
- `minimumFractionDigits`: minimum fraction digits to display
- `maximumFractionDigits`: maximum fraction digits to display
- `minimumSignificantDigits`: minimum significant digits
- `maximumSignificantDigits`: maximum significant digits
- `useGrouping`: `true` (default), `false`, `"auto"`, `"always"`, `"min2"`
- `unit`: sanctioned unit identifier (e.g. `"meter"`, `"kilometer-per-hour"`) — required when `style: "unit"`
- `unitDisplay`: `"short"` (default), `"narrow"`, or `"long"` (for `style: "unit"`)
- `signDisplay`: `"auto"` (default), `"always"`, `"never"`, `"exceptZero"`, `"negative"`
- `roundingMode`: `"halfEven"` (default), `"ceil"`, `"floor"`, `"expand"`, `"trunc"`, `"halfCeil"`, `"halfExpand"`, `"halfFloor"`, `"halfTrunc"`

Examples:
- `intl.format_number(1234567.89)` → `"1,234,567.89"`
- `intl.format_number(1234567.89, "pt-BR")` → `"1.234.567,89"`
- `intl.format_number(0.5, "en", {style: "percent"})` → `"50%"`
- `intl.format_number(1234.56, "en", {style: "currency", currency: "USD"})` → `"$1,234.56"`
- `intl.format_number(1000007, "bn")` → `"১০,০০,০০৭"`
- `intl.format_number(1000000, "en", {notation: "compact"})` → `"1M"`

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
  - `fn clone(self: &Self) -> FormatNumber`



## handl_lib_intl::format_number::FormatNumberToParts

*Struct*

`intl.format_number_to_parts(n: number, locale?: string, opts?: object) → array`

Like [`FormatNumber`], but returns an array of `{ type, value }` objects
instead of a flat string. Uses ICU4X `write_to_parts` APIs to produce
structured part annotations (e.g. `"integer"`, `"group"`, `"decimal"`,
`"fraction"`, `"minusSign"`, `"plusSign"`).

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
  - `fn clone(self: &Self) -> FormatNumberToParts`



## handl_lib_intl::format_number::build_formatter_options

*Function*

```rust
fn build_formatter_options(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> icu::decimal::options::DecimalFormatterOptions
```



## handl_lib_intl::format_number::decimal_formatter

*Function*

```rust
fn decimal_formatter(locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<std::rc::Rc<icu::decimal::DecimalFormatter>, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_compact

*Function*

```rust
fn format_compact(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, compact_display: &str, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_compact_to_parts

*Function*

```rust
fn format_compact_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, compact_display: &str, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_currency

*Function*

```rust
fn format_currency(n: f64, locale: &icu::locale::Locale, currency: &str, display: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_currency_to_parts

*Function*

```rust
fn format_currency_to_parts(n: f64, locale: &icu::locale::Locale, currency: &str, display: &str, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_decimal_to_parts

*Function*

```rust
fn format_decimal_to_parts(decimal: &fixed_decimal::Decimal, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_decimal_with_options

*Function*

```rust
fn format_decimal_with_options(decimal: &fixed_decimal::Decimal, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_engineering

*Function*

```rust
fn format_engineering(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_engineering_to_parts

*Function*

```rust
fn format_engineering_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_exponent

*Function*

```rust
fn format_exponent(exponent: i32, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_exponent_to_parts

*Function*

```rust
fn format_exponent_to_parts(exponent: i32, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_percent

*Function*

```rust
fn format_percent(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_percent_to_parts

*Function*

```rust
fn format_percent_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_scientific

*Function*

```rust
fn format_scientific(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_scientific_to_parts

*Function*

```rust
fn format_scientific_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_standard

*Function*

```rust
fn format_standard(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_standard_to_parts

*Function*

```rust
fn format_standard_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_unit

*Function*

Format a number with a unit (e.g. "5 meters", "30 km/h").
Uses ICU4X CategorizedFormatter with the available category constructors.

```rust
fn format_unit(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, _dp: &super::common::DataProvider) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_intl::format_number::format_unit_to_parts

*Function*

```rust
fn format_unit_to_parts(n: f64, locale: &icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>, _dp: &super::common::DataProvider, sink: & mut super::parts::PartsCollector) -> Result<(), ac_runtime_values::Error>
```



