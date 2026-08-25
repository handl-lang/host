**handl_typegen_key_set_special > arg**

# Module: arg

## Contents

**Functions**

- [`has_arg`](#has_arg) - Check if a field schema has an `arg` key.
- [`strip_key`](#strip_key) - Strip the `arg` key from all field schemas in the context.

---

## handl_typegen_key_set_special::arg::has_arg

*Function*

Check if a field schema has an `arg` key.

```rust
fn has_arg(schema: &serde_json::Value, vocab: &dyn DocumentVocabulary) -> bool
```



## handl_typegen_key_set_special::arg::strip_key

*Function*

Strip the `arg` key from all field schemas in the context.

```rust
fn strip_key(ctx: & mut handl_typegen_core::TypegenContext, vocab: &dyn DocumentVocabulary)
```



