**handl_typegen_key_set_special > expr**

# Module: expr

## Contents

**Functions**

- [`get_expr`](#get_expr) - Get the expression string from a field schema, if present.
- [`strip_key`](#strip_key) - Strip the `expr` key from all field schemas in the context.

---

## handl_typegen_key_set_special::expr::get_expr

*Function*

Get the expression string from a field schema, if present.

```rust
fn get_expr(schema: &serde_json::Value, vocab: &dyn DocumentVocabulary) -> Option<String>
```



## handl_typegen_key_set_special::expr::strip_key

*Function*

Strip the `expr` key from all field schemas in the context.

```rust
fn strip_key(ctx: & mut handl_typegen_core::TypegenContext, vocab: &dyn DocumentVocabulary)
```



