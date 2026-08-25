**handl_lib_intl > locale_match**

# Module: locale_match

## Contents

**Structs**

- [`GetCanonicalLocales`](#getcanonicallocales) - `intl.get_canonical_locales(locales: string | string[]) → string[]`
- [`SupportedLocalesOf`](#supportedlocalesof) - `intl.supported_locales_of(locales: string | string[]) → string[]`

**Functions**

- [`blob_supported_languages`](#blob_supported_languages)
- [`is_supported_compiled`](#is_supported_compiled)

**Constants**

- [`SUPPORTED_LANGUAGES`](#supported_languages) - Languages known to be present in the compiled ICU4X CLDR data.

---

## handl_lib_intl::locale_match::GetCanonicalLocales

*Struct*

`intl.get_canonical_locales(locales: string | string[]) → string[]`

Returns the canonicalized form of the given locale or list of locales.

Examples:
- `intl.get_canonical_locales("en-us")` → `["en-US"]`
- `intl.get_canonical_locales(["de", "ja-jp"])` → `["de", "ja-JP"]`

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> GetCanonicalLocales`



## handl_lib_intl::locale_match::SUPPORTED_LANGUAGES

*Constant*: `&[&str]`

Languages known to be present in the compiled ICU4X CLDR data.

ICU4X compiled data does not expose a public locale inventory API, so this
list is the practical fallback for `supported_locales_of` when no blob
provider is configured. With a blob provider, the actual inventory is
enumerated dynamically.



## handl_lib_intl::locale_match::SupportedLocalesOf

*Struct*

`intl.supported_locales_of(locales: string | string[]) → string[]`

Filters the requested locales to those supported by the available data.
With the default compiled provider, uses a hardcoded list of supported languages.
With a blob provider, queries the blob's actual locale inventory via
`DecimalSymbolsV1` data identifiers — does not claim support for locales
the blob doesn't contain.
Uses best-fit matching (language subtag match).

Examples:
- `intl.supported_locales_of("en")` → `["en"]`
- `intl.supported_locales_of(["de", "xxx"])` → `["de"]`

**Fields:**
- `provider: Option<super::provider::IntlProvider>`

**Methods:**

- `fn new() -> Self`
- `fn with_provider(provider: super::provider::IntlProvider) -> Self`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> SupportedLocalesOf`



## handl_lib_intl::locale_match::blob_supported_languages

*Function*

```rust
fn blob_supported_languages(provider: &super::provider::IntlProvider) -> Result<Vec<String>, ac_runtime_values::Error>
```



## handl_lib_intl::locale_match::is_supported_compiled

*Function*

```rust
fn is_supported_compiled(locale: &str) -> bool
```



