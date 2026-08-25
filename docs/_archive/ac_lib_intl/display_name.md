**handl_lib_intl > display_name**

# Module: display_name

## Contents

**Structs**

- [`DisplayName`](#displayname) - `intl.display_name(code: string, type: string, locale?: string, opts?: object) → string`

**Functions**

- [`calendar_display_name`](#calendar_display_name) - Returns a locale-aware display name for a calendar code.
- [`datetime_field_display_name`](#datetime_field_display_name) - Returns a locale-aware display name for a dateTimeField code.

---

## handl_lib_intl::display_name::DisplayName

*Struct*

`intl.display_name(code: string, type: string, locale?: string, opts?: object) → string`

Returns the display name for a language, region, script, variant, currency, calendar,
or dateTimeField code.

`type` must be one of: `"language"`, `"region"`, `"script"`, `"variant"`,
`"currency"`, `"calendar"`, or `"dateTimeField"`.

Options:
- `style`: `"long"` (default), `"short"`, or `"narrow"`
- `fallback`: `"code"` (default) or `"none"` (returns error if not found)
- `languageDisplay`: `"dialect"` (default) or `"standard"` (for `type: "language"`)

**Note:** `calendar` and `dateTimeField` types return English-only display names
using static CLDR-based fallbacks. ICU4X does not yet expose locale-aware APIs
for these types. The `locale` argument is ignored for these types.

Examples:
- `intl.display_name("en", "language", "pt")` → `"Inglês"`
- `intl.display_name("US", "region", "en")` → `"United States"`
- `intl.display_name("US", "region", "en", {style: "short"})` → `"US"`
- `intl.display_name("Latn", "script", "en")` → `"Latin"`
- `intl.display_name("fonipa", "variant", "en")` → `"IPA Phonetics"`

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
  - `fn clone(self: &Self) -> DisplayName`



## handl_lib_intl::display_name::calendar_display_name

*Function*

Returns a locale-aware display name for a calendar code.
Uses static CLDR-based English names as fallback (ICU4X does not yet expose calendar display name APIs).

```rust
fn calendar_display_name(code: &str, _style: &str) -> Option<String>
```



## handl_lib_intl::display_name::datetime_field_display_name

*Function*

Returns a locale-aware display name for a dateTimeField code.
Uses static CLDR-based English names as fallback (ICU4X does not yet expose dateTimeField display name APIs).

```rust
fn datetime_field_display_name(code: &str, _style: &str) -> Option<String>
```



