**handl_compiler > source**

# Module: source

## Contents

**Structs**

- [`Source`](#source) - Source input for a compilation.

---

## handl_compiler::source::Source

*Struct*

Source input for a compilation.

**Fields:**
- `input: String` - Raw input text.
- `format: String` - Format name (`yaml`, `json`, `toml`).
- `path: Option<std::path::PathBuf>` - File path of the input, if known. Used for relative import resolution.

**Methods:**

- `fn new<impl Into<String>, impl Into<String>>(input: impl Trait, format: impl Trait) -> Self`
- `fn with_path<impl Into<PathBuf>>(self: Self, path: impl Trait) -> Self`
- `fn base_path(self: &Self) -> Option<PathBuf>` - Base path for resolving relative imports — the parent directory

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Source`



