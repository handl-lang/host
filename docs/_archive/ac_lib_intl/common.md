**handl_lib_intl > common**

# Module: common

## Contents

**Enums**

- [`DataProvider`](#dataprovider) - A data provider that is either compiled (static) or blob-backed (runtime).

**Functions**

- [`apply_digit_options`](#apply_digit_options)
- [`apply_fraction_digits`](#apply_fraction_digits)
- [`apply_minimum_integer_digits`](#apply_minimum_integer_digits)
- [`apply_numbering_system`](#apply_numbering_system) - Apply a `numberingSystem` option to a locale by setting the Unicode `-u-nu-<value>` extension.
- [`apply_sign_display`](#apply_sign_display)
- [`apply_significant_digits`](#apply_significant_digits)
- [`decimal_from_f64`](#decimal_from_f64) - Convert an f64 into an ICU4X `Decimal` using `FloatPrecision::RoundTrip`
- [`make_provider`](#make_provider) - Create a `DataProvider` from an optional `IntlProvider`.
- [`opt_bool`](#opt_bool) - Get a boolean field from an options object.
- [`opt_int`](#opt_int) - Get an integer field from an options object.
- [`opt_str`](#opt_str) - Get a string field from an options object.
- [`optional_locale`](#optional_locale) - Extract an optional locale argument at the given index.
- [`optional_object`](#optional_object) - Extract an optional options object at the given index.
- [`parse_rounding_mode`](#parse_rounding_mode)
- [`require_array`](#require_array) - Require an array argument at `idx`.
- [`require_number`](#require_number) - Require a number argument at `idx`.
- [`require_object`](#require_object) - Require an object argument at `idx`.
- [`require_string`](#require_string) - Require a string argument at `idx`.
- [`s`](#s)

---

## handl_lib_intl::common::DataProvider

*Enum*

A data provider that is either compiled (static) or blob-backed (runtime).

**Variants:**
- `Compiled`
- `Blob(usize, icu_provider_blob::BlobDataProvider)`

**Methods:**

- `fn as_buffer_provider(self: &Self) -> Option<&BlobDataProvider>`
- `fn provider_id(self: &Self) -> usize`



## handl_lib_intl::common::apply_digit_options

*Function*

```rust
fn apply_digit_options(decimal: & mut fixed_decimal::Decimal, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::apply_fraction_digits

*Function*

```rust
fn apply_fraction_digits(decimal: & mut fixed_decimal::Decimal, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::apply_minimum_integer_digits

*Function*

```rust
fn apply_minimum_integer_digits(decimal: & mut fixed_decimal::Decimal, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::apply_numbering_system

*Function*

Apply a `numberingSystem` option to a locale by setting the Unicode `-u-nu-<value>` extension.

```rust
fn apply_numbering_system(locale: & mut icu::locale::Locale, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::apply_sign_display

*Function*

```rust
fn apply_sign_display(decimal: & mut fixed_decimal::Decimal, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::apply_significant_digits

*Function*

```rust
fn apply_significant_digits(decimal: & mut fixed_decimal::Decimal, opts: Option<&serde_json::Map<String, serde_json::Value>>)
```



## handl_lib_intl::common::decimal_from_f64

*Function*

Convert an f64 into an ICU4X `Decimal` using `FloatPrecision::RoundTrip`
for exact round-trip representation (avoids `0.1 + 0.2` → `"0.30000000000000004"`).

```rust
fn decimal_from_f64(n: f64) -> fixed_decimal::Decimal
```



## handl_lib_intl::common::make_provider

*Function*

Create a `DataProvider` from an optional `IntlProvider`.

```rust
fn make_provider(provider: &Option<super::provider::IntlProvider>) -> Result<DataProvider, ac_runtime_values::Error>
```



## handl_lib_intl::common::opt_bool

*Function*

Get a boolean field from an options object.
Accepts both JSON booleans and string "true"/"false".

```rust
fn opt_bool(obj: Option<&serde_json::Map<String, serde_json::Value>>, key: &str) -> Option<bool>
```



## handl_lib_intl::common::opt_int

*Function*

Get an integer field from an options object.
Accepts both JSON numbers and numeric strings.

```rust
fn opt_int(obj: Option<&serde_json::Map<String, serde_json::Value>>, key: &str) -> Option<i64>
```



## handl_lib_intl::common::opt_str

*Function*

Get a string field from an options object.

```rust
fn opt_str<'a>(obj: Option<&'a serde_json::Map<String, serde_json::Value>>, key: &str) -> Option<&'a str>
```



## handl_lib_intl::common::optional_locale

*Function*

Extract an optional locale argument at the given index.
Defaults to `en` if not present.

```rust
fn optional_locale(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<icu::locale::Locale, ac_runtime_values::Error>
```



## handl_lib_intl::common::optional_object

*Function*

Extract an optional options object at the given index.

```rust
fn optional_object(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<Option<&serde_json::Map<String, serde_json::Value>>, ac_runtime_values::Error>
```



## handl_lib_intl::common::parse_rounding_mode

*Function*

```rust
fn parse_rounding_mode(opts: Option<&serde_json::Map<String, serde_json::Value>>) -> fixed_decimal::SignedRoundingMode
```



## handl_lib_intl::common::require_array

*Function*

Require an array argument at `idx`.

```rust
fn require_array(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_intl::common::require_number

*Function*

Require a number argument at `idx`.

```rust
fn require_number(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_intl::common::require_object

*Function*

Require an object argument at `idx`.

```rust
fn require_object(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&serde_json::Map<String, serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_intl::common::require_string

*Function*

Require a string argument at `idx`.

```rust
fn require_string(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_intl::common::s

*Function*

```rust
fn s<impl Into<String>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



