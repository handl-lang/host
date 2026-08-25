**handl_source > sourcemap**

# Module: sourcemap

## Contents

**Structs**

- [`SourceMap`](#sourcemap) - Maps dotted paths (e.g. `"server.port"`) to [`SourceLocation`]s.

---

## handl_source::sourcemap::SourceMap

*Struct*

Maps dotted paths (e.g. `"server.port"`) to [`SourceLocation`]s.

Pure data structure — populated by each frontend's `build_source_map`
function using format-specific scanning. `handl-source` itself knows
nothing about YAML, JSON, or TOML syntax.

# Example

```
use handl_source::{SourceMap, SourceLocation};

let mut map = SourceMap::new();
map.insert("server", SourceLocation::new(1, 1));
map.insert("server.port", SourceLocation::new(2, 3));

assert_eq!(map.resolve_path("server.port"), Some(SourceLocation::new(2, 3)));
assert_eq!(map.resolve_path("unknown"), None);
```

**Fields:**
- `map: std::collections::HashMap<String, crate::SourceLocation>`

**Methods:**

- `fn new() -> Self` - Creates an empty `SourceMap`.
- `fn insert<impl Into<String>>(self: & mut Self, path: impl Trait, location: SourceLocation)` - Inserts a path → location mapping.
- `fn resolve_path(self: &Self, path: &str) -> Option<SourceLocation>` - Resolves a dotted path to a [`SourceLocation`].
- `fn len(self: &Self) -> usize` - Returns the number of entries.
- `fn is_empty(self: &Self) -> bool` - Returns true if the map is empty.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



