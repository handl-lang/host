# ac_typecheck_key_mutable

# handl-typecheck-key-mutable

Typechecks the `mutable` handl key.

Fields with `mutable: false` cannot be overridden in nested scopes.
This crate walks the data tree and reports any nested overrides of
immutable fields as [`TypeError`]s.

## Architectural rule

> **Immutable fields cannot be overridden in nested scopes.**

## Modules

### [`ac_typecheck_key_mutable`](ac_typecheck_key_mutable.md)

*1 constant, 2 functions*

