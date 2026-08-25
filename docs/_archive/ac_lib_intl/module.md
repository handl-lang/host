**handl_lib_intl > module**

# Module: module

## Contents

**Structs**

- [`IntlModule`](#intlmodule) - The `intl` module — internationalization functions.

---

## handl_lib_intl::module::IntlModule

*Struct*

The `intl` module — internationalization functions.

Provides functions callable as `intl.format_number(n, locale?, opts?)`,
`intl.select_plural(n, locale?, opts?)`,
`intl.format_list(items, locale?, opts?)`,
`intl.format_relative_time(value, unit, locale?, opts?)`,
`intl.format_date(date, locale?, opts?)`,
`intl.format_duration(duration, locale?, opts?)`,
`intl.compare_strings(a, b, locale?, opts?)`,
`intl.display_name(code, type, locale?, opts?)`,
`intl.segment(text, granularity, locale?, opts?)`,
`intl.supported_locales_of()` in handl expressions.

All functions accept an optional locale argument (BCP 47 tag).
If omitted, defaults to English.

By default, uses ICU4X compiled CLDR data (600+ locales).
For smaller binaries, construct with [`IntlModule::with_provider`]
to use a runtime-loaded data blob.

**Fields:**
- `provider: Option<super::provider::IntlProvider>`
- `f_format_number: super::format_number::FormatNumber`
- `f_format_number_to_parts: super::format_number::FormatNumberToParts`
- `f_select_plural: super::select_plural::SelectPlural`
- `f_format_list: super::format_list::FormatList`
- `f_format_relative_time: super::format_relative_time::FormatRelativeTime`
- `f_format_date: super::format_date::FormatDate`
- `f_format_date_to_parts: super::format_date::FormatDateToParts`
- `f_format_duration: super::format_duration::FormatDuration`
- `f_compare_strings: super::compare_strings::CompareStrings`
- `f_display_name: super::display_name::DisplayName`
- `f_segment: super::segment::Segment`
- `f_supported_locales_of: super::locale_match::SupportedLocalesOf`
- `f_get_canonical_locales: super::locale_match::GetCanonicalLocales`
- `f_locale: super::locale::LocaleInfo`

**Methods:**

- `fn new() -> Self` - Create a new `IntlModule` using ICU4X compiled CLDR data.
- `fn with_provider(provider: IntlProvider) -> Self` - Create a new `IntlModule` using a runtime-loaded data blob.
- `fn with_blob_provider(blob: Vec<u8>) -> Self` - Create a new `IntlModule` from a blob of ICU4X postcard data.
- `fn provider(self: &Self) -> Option<&IntlProvider>` - Returns the data provider, if one was set.

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> IntlModule`



