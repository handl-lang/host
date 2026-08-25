**handl_dialect_canonical > dialect > dialect**

# Module: dialect::dialect

## Contents

**Structs**

- [`HandlDialect`](#acdialect) - The canonical handl language dialect.

**Statics**

- [`AC_DIALECT`](#handl_dialect) - The canonical handl dialect as a static singleton.

---

## handl_dialect_canonical::dialect::dialect::AC_DIALECT

*Static*

The canonical handl dialect as a static singleton.

```rust
static AC_DIALECT: HandlDialect
```



## handl_dialect_canonical::dialect::dialect::HandlDialect

*Struct*

The canonical handl language dialect.

Implements the syntax defined in the handl Language Specification.
See the crate-level documentation for the full list of canonical values.

**Unit Struct**

**Methods:**

- `fn op_name_end(self: &Self, s: &str) -> Option<usize>` - Find the end of the leading string literal in `s`.

**Traits:** Copy

**Trait Implementations:**

- **Syntax**
  - `fn interpolation_prefix(self: &Self) -> &str`
  - `fn interpolation_suffix(self: &Self) -> &str`
  - `fn path_separator(self: &Self) -> char`
  - `fn call_open(self: &Self) -> char`
  - `fn call_close(self: &Self) -> char`
- **Clone**
  - `fn clone(self: &Self) -> HandlDialect`
- **Dialect**
  - `fn syntax(self: &Self) -> &dyn Syntax`
  - `fn parser(self: &Self) -> &dyn ExprParser`
  - `fn document_vocabulary(self: &Self) -> &dyn DocumentVocabulary`
  - `fn schema_vocabulary(self: &Self) -> &dyn SchemaVocabulary`
- **ExprParser**
  - `fn find_call_open(self: &Self, s: &str) -> Option<usize>`
  - `fn split_args(self: &Self, s: &str) -> Vec<String>`
  - `fn parse_string_literal(self: &Self, s: &str) -> Option<String>`
  - `fn parse_number_literal(self: &Self, s: &str) -> Option<f64>`
  - `fn parse_bool_literal(self: &Self, s: &str) -> Option<bool>`
  - `fn is_null_literal(self: &Self, s: &str) -> bool`
  - `fn find_handle_open(self: &Self, s: &str) -> Option<usize>`
  - `fn find_block_open(self: &Self, s: &str) -> Option<usize>`
  - `fn split_handler_clauses(self: &Self, s: &str) -> Vec<String>`
  - `fn parse_handler_clause(self: &Self, s: &str) -> Option<ParsedHandlerClause>`
- **DocumentVocabulary**
  - `fn fields_key(self: &Self) -> &str`
  - `fn expr_key(self: &Self) -> &str`
  - `fn arg_key(self: &Self) -> &str`
  - `fn merge_key(self: &Self) -> &str`
  - `fn inheritable_key(self: &Self) -> &str`
  - `fn mutable_key(self: &Self) -> &str`
  - `fn params_key(self: &Self) -> &str`
  - `fn return_key(self: &Self) -> &str`
  - `fn body_key(self: &Self) -> &str`



