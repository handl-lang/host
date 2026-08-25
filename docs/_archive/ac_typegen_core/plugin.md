**handl_typegen_core > plugin**

# Module: plugin

## Contents

**Functions**

- [`get_expr`](#get_expr) - Read the expression string from a field schema, if present.

**Traits**

- [`ImportResolver`](#importresolver) - Resolves imports in field schemas during type generation.
- [`KeyHandler`](#keyhandler) - Handles special keys in field schemas during type generation.

---

## handl_typegen_core::plugin::ImportResolver

*Trait*

Resolves imports in field schemas during type generation.

Implementations detect import expressions in merge fields, follow
import paths via a [`ModuleResolver`], and merge imported schemas
into the parent context (parent wins on conflicts).

The canonical implementation lives in `handl-typegen-import`.

**Methods:**

- `resolve_imports`: Detect and resolve imports from merge fields, merging imported



## handl_typegen_core::plugin::KeyHandler

*Trait*

Handles special keys in field schemas during type generation.

Implementations know which keys are special and how to process them
(filter merge fields, strip special keys). The core provides generic
primitives (`TypegenContext::remove_fields`, `TypegenContext::strip_keys`);
the handler decides which keys to act on and when.

The canonical implementation lives in `handl-typegen-key-set-special`.

**Methods:**

- `filter_fields`: Remove fields that should be excluded from the output schema
- `strip_keys`: Strip all special keys from field schemas in the context.



## handl_typegen_core::plugin::get_expr

*Function*

Read the expression string from a field schema, if present.

Uses the vocabulary's `expr_key` to look up the expression value.

```rust
fn get_expr(schema: &serde_json::Value, vocab: &dyn DocumentVocabulary) -> Option<String>
```



