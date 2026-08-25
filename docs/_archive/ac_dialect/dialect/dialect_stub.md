**handl_dialect > dialect > dialect_stub**

# Module: dialect::dialect_stub

## Contents

**Structs**

- [`StubDialect`](#stubdialect) - A minimal dialect implementation for testing.

**Functions**

- [`stub_vocab`](#stub_vocab) - Access the stub's [`DocumentVocabulary`] without importing the [`Dialect`] trait.

**Statics**

- [`STUB_DIALECT`](#stub_dialect) - A static [`StubDialect`] instance for convenience.

---

## handl_dialect::dialect::dialect_stub::STUB_DIALECT

*Static*

A static [`StubDialect`] instance for convenience.

```rust
static STUB_DIALECT: StubDialect
```



## handl_dialect::dialect::dialect_stub::StubDialect

*Struct*

A minimal dialect implementation for testing.

**Unit Struct**

**Trait Implementations:**

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
- **Dialect**
  - `fn syntax(self: &Self) -> &dyn Syntax`
  - `fn parser(self: &Self) -> &dyn ExprParser`
  - `fn document_vocabulary(self: &Self) -> &dyn DocumentVocabulary`
  - `fn schema_vocabulary(self: &Self) -> &dyn SchemaVocabulary`
- **Syntax**
  - `fn interpolation_prefix(self: &Self) -> &str`
  - `fn interpolation_suffix(self: &Self) -> &str`
  - `fn path_separator(self: &Self) -> char`
  - `fn call_open(self: &Self) -> char`
  - `fn call_close(self: &Self) -> char`
- **ExprParser**
  - `fn find_call_open(self: &Self, s: &str) -> Option<usize>`
  - `fn split_args(self: &Self, s: &str) -> Vec<String>`
  - `fn parse_string_literal(self: &Self, s: &str) -> Option<String>`
  - `fn parse_number_literal(self: &Self, s: &str) -> Option<f64>`
  - `fn parse_bool_literal(self: &Self, s: &str) -> Option<bool>`
  - `fn is_null_literal(self: &Self, s: &str) -> bool`



## handl_dialect::dialect::dialect_stub::stub_vocab

*Function*

Access the stub's [`DocumentVocabulary`] without importing the [`Dialect`] trait.

```rust
fn stub_vocab() -> &'static dyn DocumentVocabulary
```



