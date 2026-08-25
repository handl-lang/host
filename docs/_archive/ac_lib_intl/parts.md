**handl_lib_intl > parts**

# Module: parts

## Contents

**Structs**

- [`PartsCollector`](#partscollector) - A `PartsWrite` implementation that collects formatted output into

---

## handl_lib_intl::parts::PartsCollector

*Struct*

A `PartsWrite` implementation that collects formatted output into
`(text, part)` pairs, suitable for building `formatToParts`-style
structured results.

Text written outside any `with_part` call is tagged with
`Part { category: "", value: "literal" }`.

**Fields:**
- `parts: Vec<(String, writeable::Part)>`
- `buf: String`
- `current_part: writeable::Part`

**Methods:**

- `fn new() -> Self`
- `fn flush(self: & mut Self)`
- `fn into_parts(self: Self) -> Vec<(String, String)>` - Returns the collected parts as `(type, value)` pairs, where
- `fn into_value(self: Self) -> NonFunctionValue` - Returns the collected parts as a `NonFunctionValue` array of

**Trait Implementations:**

- **PartsWrite**
  - `fn with_part<impl FnMut(&mut Self::SubPartsWrite) -> fmt::Result>(self: & mut Self, part: Part, f: impl Trait) -> fmt::Result`
- **Write**
  - `fn write_str(self: & mut Self, s: &str) -> fmt::Result`
  - `fn write_char(self: & mut Self, c: char) -> fmt::Result`
- **Default**
  - `fn default() -> Self`



