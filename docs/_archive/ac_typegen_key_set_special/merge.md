**handl_typegen_key_set_special > merge**

# Module: merge

## Contents

**Functions**

- [`filter_fields`](#filter_fields) - Remove merge fields from the context by name.
- [`is_merge_field`](#is_merge_field) - Check if a field schema has `merge: true`.
- [`strip_key`](#strip_key) - Strip the `merge` key from all field schemas in the context.

---

## handl_typegen_key_set_special::merge::filter_fields

*Function*

Remove merge fields from the context by name.

Merge fields are excluded from the output schema — they're spread
into the parent scope at runtime and removed.

```rust
fn filter_fields(ctx: & mut handl_typegen_core::TypegenContext, merge_names: &[String])
```



## handl_typegen_key_set_special::merge::is_merge_field

*Function*

Check if a field schema has `merge: true`.

```rust
fn is_merge_field(schema: &serde_json::Value, vocab: &dyn DocumentVocabulary) -> bool
```



## handl_typegen_key_set_special::merge::strip_key

*Function*

Strip the `merge` key from all field schemas in the context.

```rust
fn strip_key(ctx: & mut handl_typegen_core::TypegenContext, vocab: &dyn DocumentVocabulary)
```



