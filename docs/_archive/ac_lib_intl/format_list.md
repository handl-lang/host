**handl_lib_intl > format_list**

# Module: format_list

## Contents

**Structs**

- [`FormatList`](#formatlist) - `intl.format_list(items: array, locale?: string, opts?: object) → string`

---

## handl_lib_intl::format_list::FormatList

*Struct*

`intl.format_list(items: array, locale?: string, opts?: object) → string`

Formats a list of strings using ICU4X.

Options:
- `type`: `"conjunction"` (default), `"disjunction"`, or `"unit"`
- `style`: `"long"` (default), `"short"`, or `"narrow"`

Examples:
- `intl.format_list(["a", "b", "c"])` → `"a, b, and c"`
- `intl.format_list(["a", "b", "c"], "en", {type: "disjunction"})` → `"a, b, or c"`
- `intl.format_list(["5", "7"], "en", {type: "unit"})` → `"5, 7"`

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
  - `fn clone(self: &Self) -> FormatList`



