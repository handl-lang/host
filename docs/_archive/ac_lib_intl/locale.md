**handl_lib_intl > locale**

# Module: locale

## Contents

**Structs**

- [`LocaleInfo`](#localeinfo) - `intl.locale(tag: string, opts?: object) → object`

---

## handl_lib_intl::locale::LocaleInfo

*Struct*

`intl.locale(tag: string, opts?: object) → object`

Parses a BCP 47 locale tag and returns an object with its components.

This is equivalent to `Intl.Locale` in the ECMA-402 spec.

Options:
- `language`: override the language subtag
- `script`: override the script subtag
- `region`: override the region subtag
- `calendar`: add a `-u-ca-` extension
- `numberingSystem`: add a `-u-nu-` extension
- `hourCycle`: add a `-u-hc-` extension (`"h11"`, `"h12"`, `"h23"`, `"h24"`)

Returns an object with:
- `baseName`: the canonical locale identifier (e.g. `"en-US"`)
- `language`: the language subtag (e.g. `"en"`)
- `script`: the script subtag (e.g. `"Latn"`), or `null`
- `region`: the region subtag (e.g. `"US"`), or `null`
- `calendar`: the calendar extension, or `null`
- `numberingSystem`: the numbering system extension, or `null`
- `hourCycle`: the hour cycle extension, or `null`

Examples:
- `intl.locale("en-US")` → `{baseName: "en-US", language: "en", script: null, region: "US", ...}`
- `intl.locale("zh-Hant-TW")` → `{baseName: "zh-Hant-TW", language: "zh", script: "Hant", region: "TW", ...}`
- `intl.locale("en", {region: "GB"})` → `{baseName: "en-GB", language: "en", script: null, region: "GB", ...}`

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> LocaleInfo`



