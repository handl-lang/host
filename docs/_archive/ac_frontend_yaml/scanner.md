**handl_frontend_yaml > scanner**

# Module: scanner

## Contents

**Functions**

- [`build_source_map`](#build_source_map) - Builds a `SourceMap` for YAML source text.
- [`find_key_location`](#find_key_location)
- [`find_path_offset`](#find_path_offset)

**Constants**

- [`MATCHER`](#matcher)

---

## handl_frontend_yaml::scanner::MATCHER

*Constant*: `handl_source::KeyMatcher`



## handl_frontend_yaml::scanner::build_source_map

*Function*

Builds a `SourceMap` for YAML source text.

Scans the text for `key:` patterns at various indentation levels,
matching them against the structure of the parsed value to build
accurate path → location mappings.

```rust
fn build_source_map(input: &str, value: &serde_json::Value) -> handl_source::SourceMap
```



## handl_frontend_yaml::scanner::find_key_location

*Function*

```rust
fn find_key_location(source: &str, lines: &handl_source::LineIndex, key: &str, parent_path: &str) -> Option<handl_source::SourceLocation>
```



## handl_frontend_yaml::scanner::find_path_offset

*Function*

```rust
fn find_path_offset(source: &str, lines: &handl_source::LineIndex, path: &str) -> Option<usize>
```



