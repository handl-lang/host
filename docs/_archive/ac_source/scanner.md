**handl_source > scanner**

# Module: scanner

## Contents

**Structs**

- [`KeyFound`](#keyfound) - Result of finding a key on a line.
- [`KeyMatcher`](#keymatcher) - Configuration for key matching on a line.
- [`LineIndex`](#lineindex) - Index of line-start byte offsets within a source string.

**Functions**

- [`leading_whitespace`](#leading_whitespace) - Count leading space and tab characters in a string.
- [`walk_value`](#walk_value) - Walk a parsed `serde_json::Value` tree, calling `on_key` for each

---

## handl_source::scanner::KeyFound

*Struct*

Result of finding a key on a line.

**Fields:**
- `key_offset: usize` - Byte offset of the key start within the source.
- `value_offset: usize` - Byte offset of the value start (after the separator).
- `indent: usize` - Indentation level (leading whitespace) of the line.



## handl_source::scanner::KeyMatcher

*Struct*

Configuration for key matching on a line.

**Fields:**
- `separator: char` - The separator character after the key (`:` for YAML/JSON, `=` for TOML).

**Methods:**

- `fn match_key(self: &Self, line: &str, key: &str) -> Option<usize>` - Match a key at the start of a line, supporting unquoted,
- `fn find_key_in_lines(self: &Self, source: &str, lines: &LineIndex, key: &str, from: usize, min_indent: Option<usize>, skip_from_line: bool) -> Option<KeyFound>` - Search for a key in the source text starting from a given byte offset,



## handl_source::scanner::LineIndex

*Struct*

Index of line-start byte offsets within a source string.

Precomputed from the source text so that `O(log n)` line/column
lookups are possible from any byte offset.

**Fields:**
- `starts: Vec<usize>`
- `source_len: usize`

**Methods:**

- `fn new(source: &str) -> Self` - Build a line index from source text.
- `fn at_offset(self: &Self, offset: usize) -> SourceLocation` - Convert a byte offset into a 1-indexed `SourceLocation`.
- `fn starts(self: &Self) -> &[usize]` - Returns the line-start byte offsets.
- `fn line_start(self: &Self, line_idx: usize) -> usize` - Returns the byte offset where the given line (0-indexed) begins.
- `fn line_end(self: &Self, line_idx: usize) -> usize` - Returns the byte offset where the given line (0-indexed) ends



## handl_source::scanner::leading_whitespace

*Function*

Count leading space and tab characters in a string.

```rust
fn leading_whitespace(s: &str) -> usize
```



## handl_source::scanner::walk_value

*Function*

Walk a parsed `serde_json::Value` tree, calling `on_key` for each
object key with its dotted path and byte-offset search context.

The `on_key` callback receives:
- `path` — the dotted path to the key (e.g. `"server.port"`)
- `key` — the key name
- `parent_path` — the dotted path of the parent (empty for root)

If `on_key` returns `Some(SourceLocation)`, it is inserted into the
`SourceMap`.

```rust
fn walk_value<impl FnMut(&str, &str, &str) -> Option<SourceLocation>>(value: &serde_json::Value, path: &str, on_key: & mut impl Trait, map: & mut crate::SourceMap)
```



