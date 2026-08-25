**handl_lexer_common > tokens > factory**

# Module: tokens::factory

## Contents

**Structs**

- [`MockFactory`](#mockfactory)

---

## handl_lexer_common::tokens::factory::MockFactory

*Struct*

**Fields:**
- `string_fn: std::sync::Arc<dyn Fn>`
- `number_fn: std::sync::Arc<dyn Fn>`
- `ident_fn: std::sync::Arc<dyn Fn>`
- `comment_fn: std::sync::Arc<dyn Fn>`
- `keyword_fn: std::sync::Arc<dyn Fn>`
- `operator_fn: std::sync::Arc<dyn Fn>`

**Methods:**

- `fn new() -> Self`
- `fn with_string_kind<F>(self: Self, f: F) -> Self`
- `fn with_number_kind<F>(self: Self, f: F) -> Self`
- `fn with_ident_kind<F>(self: Self, f: F) -> Self`
- `fn with_comment_kind<F>(self: Self, f: F) -> Self`
- `fn with_keyword_kind<F>(self: Self, f: F) -> Self`
- `fn with_operator_kind<F>(self: Self, f: F) -> Self`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Self`
- **Default**
  - `fn default() -> Self`
- **CommentFactory**
  - `fn comment_kind(self: &Self, text: String) -> MockKind`
- **KindFactory**
  - `fn string_kind(self: &Self, value: String) -> MockKind`
  - `fn number_kind(self: &Self, value: f64) -> MockKind`
  - `fn ident_kind(self: &Self, text: String) -> MockKind`
  - `fn eof_kind(self: &Self) -> MockKind`
  - `fn keyword_kind(self: &Self, text: &str) -> Option<MockKind>`
  - `fn operator_kind(self: &Self, text: &str) -> Option<MockKind>`



