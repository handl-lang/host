**handl_lib_intl > compare_strings**

# Module: compare_strings

## Contents

**Structs**

- [`CompareStrings`](#comparestrings) - `intl.compare_strings(a: string, b: string, locale?: string, opts?: object) → number`

---

## handl_lib_intl::compare_strings::CompareStrings

*Struct*

`intl.compare_strings(a: string, b: string, locale?: string, opts?: object) → number`

Compares two strings using ICU4X collation.

Returns `-1` if `a < b`, `0` if `a == b`, `1` if `a > b`.

Options:
- `sensitivity`: `"base"`, `"accent"`, `"case"`, or `"variant"` (default)
- `caseFirst`: `"upper"`, `"lower"`, or `"false"` (default)
- `numeric`: `true` or `false` (default) — sorts digit sequences by numeric value
- `ignorePunctuation`: `true` or `false` (default)
- `usage`: `"sort"` (default) or `"search"` — selects sort vs search collation
- `collation`: collation variant string (e.g. `"phonebk"` for German phonebook)

Examples:
- `intl.compare_strings("a", "b")` → `-1`
- `intl.compare_strings("é", "e", "en", {sensitivity: "base"})` → `0`
- `intl.compare_strings("file10", "file2", "en", {numeric: true})` → `1`

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
  - `fn clone(self: &Self) -> CompareStrings`



