**handl_lib_intl > select_plural**

# Module: select_plural

## Contents

**Structs**

- [`SelectPlural`](#selectplural) - `intl.select_plural(n: number, locale?: string, opts?: object) → string`

---

## handl_lib_intl::select_plural::SelectPlural

*Struct*

`intl.select_plural(n: number, locale?: string, opts?: object) → string`

Returns the CLDR plural category for a number in the given locale.

Options:
- `type`: `"cardinal"` (default) or `"ordinal"`
- `minimumIntegerDigits`: minimum integer digits
- `minimumFractionDigits`: minimum fraction digits to display
- `maximumFractionDigits`: maximum fraction digits to display
- `minimumSignificantDigits`: minimum significant digits
- `maximumSignificantDigits`: maximum significant digits

Examples:
- `intl.select_plural(1)` → `"one"`
- `intl.select_plural(5)` → `"other"`
- `intl.select_plural(0, "ar")` → `"zero"`
- `intl.select_plural(2, "en", {type: "ordinal"})` → `"two"`
- `intl.select_plural(11, "en", {type: "ordinal"})` → `"other"`
- `intl.select_plural(1.0, "en")` → `"other"` (trailing zero → not "one")
- `intl.select_plural(1, "en", {minimumFractionDigits: 2})` → `"other"` (padded to 1.00)

**Fields:**
- `provider: Option<super::provider::IntlProvider>`

**Methods:**

- `fn new() -> Self`
- `fn with_provider(provider: IntlProvider) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> SelectPlural`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



