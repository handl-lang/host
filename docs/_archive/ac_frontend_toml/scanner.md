**handl_frontend_toml > scanner**

# Module: scanner

## Contents

**Structs**

- [`Section`](#section)

**Functions**

- [`build_source_map`](#build_source_map) - Builds a `SourceMap` for TOML source text.
- [`find_sections`](#find_sections) - Finds all `[section]` and `[[section]]` headers in the TOML text.
- [`find_toml_key`](#find_toml_key) - Finds a `key = value` line in the TOML source after the given offset.

**Constants**

- [`MATCHER`](#matcher)

---

## handl_frontend_toml::scanner::MATCHER

*Constant*: `handl_source::KeyMatcher`



## handl_frontend_toml::scanner::Section

*Struct*

**Fields:**
- `path: String` - Dotted path of the section (e.g. "server" or "server.ssl")
- `line_start: usize` - Byte offset of the section header line start



## handl_frontend_toml::scanner::build_source_map

*Function*

Builds a `SourceMap` for TOML source text.

```rust
fn build_source_map(input: &str, value: &serde_json::Value) -> handl_source::SourceMap
```



## handl_frontend_toml::scanner::find_sections

*Function*

Finds all `[section]` and `[[section]]` headers in the TOML text.

```rust
fn find_sections(source: &str, lines: &handl_source::LineIndex) -> Vec<Section>
```



## handl_frontend_toml::scanner::find_toml_key

*Function*

Finds a `key = value` line in the TOML source after the given offset.

```rust
fn find_toml_key(source: &str, lines: &handl_source::LineIndex, key: &str, from: usize) -> Option<handl_source::SourceLocation>
```



