**handl_lib_intl > segment**

# Module: segment

## Contents

**Structs**

- [`Segment`](#segment) - `intl.segment(text: string, granularity: string, locale?: string, opts?: object) → array`

**Functions**

- [`collect_segments`](#collect_segments)
- [`collect_word_segments`](#collect_word_segments)

---

## handl_lib_intl::segment::Segment

*Struct*

`intl.segment(text: string, granularity: string, locale?: string, opts?: object) → array`

Segments text using ICU4X.

`granularity` must be one of: `"grapheme"`, `"word"`, `"sentence"`, or `"line"`.

Returns an array of segment objects with `segment` (string), `index` (number),
and `isWordLike` (boolean, only for word granularity).

Options (for `granularity: "line"`):
- `lineBreakStyle`: `"strict"`, `"normal"`, `"loose"`, or `"anywhere"` (default: ICU4X default)
- `lineBreakWord`: `"normal"`, `"break-all"`, or `"keep-all"` (default: `"normal"`)

Examples:
- `intl.segment("hello world", "word")` → `[{segment: "hello", index: 0, isWordLike: true}, ...]`
- `intl.segment("こんにちは", "grapheme", "ja")` → 5 segments
- `intl.segment("Hello World", "line", "ja", {lineBreakStyle: "loose"})` → line segments with loose breaking

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
  - `fn clone(self: &Self) -> Segment`



## handl_lib_intl::segment::collect_segments

*Function*

```rust
fn collect_segments<I>(text: &str, breakpoints: I) -> Vec<serde_json::Value>
```



## handl_lib_intl::segment::collect_word_segments

*Function*

```rust
fn collect_word_segments<I>(text: &str, breakpoints: I) -> Vec<serde_json::Value>
```



