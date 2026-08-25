# ac_typecheck_key_merge

# handl-typecheck-key-merge

Typechecks the `merge` handl key.

Fields with `merge: true` are objects whose properties are spread
into the parent scope at runtime. This crate checks that:

- Merge fields have type `object` (merging a non-object is a type error).
- Merged field types are compatible with the parent's declared types
  for the same field names.

## Architectural rule

> **Merge fields must be objects, and their properties must be
> type-compatible with the parent scope.**

## Modules

### [`ac_typecheck_key_merge`](ac_typecheck_key_merge.md)

*1 constant, 3 functions*

