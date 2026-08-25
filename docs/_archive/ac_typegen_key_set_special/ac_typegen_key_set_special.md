**handl_typegen_key_set_special**

# Module: handl_typegen_key_set_special

## Contents

**Modules**

- [`arg`](#arg)
- [`expr`](#expr)
- [`inheritable`](#inheritable)
- [`merge`](#merge)
- [`mutable`](#mutable)

**Structs**

- [`SpecialKeyHandler`](#specialkeyhandler) - Key handler for the standard handl special key set.

---

## handl_typegen_key_set_special::SpecialKeyHandler

*Struct*

Key handler for the standard handl special key set.

Implements [`KeyHandler`] using the 5 canonical handl keys (`arg`, `expr`,
`inheritable`, `merge`, `mutable`). Key names are resolved via
[`DocumentVocabulary`], so custom dialects with different key names
work without changes.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Default**
  - `fn default() -> SpecialKeyHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> SpecialKeyHandler`
- **KeyHandler**
  - `fn filter_fields(self: &Self, ctx: & mut TypegenContext, merge_names: &[String])`
  - `fn strip_keys(self: &Self, ctx: & mut TypegenContext, vocab: &dyn DocumentVocabulary)`



## Module: arg



## Module: expr



## Module: inheritable



## Module: merge



## Module: mutable



