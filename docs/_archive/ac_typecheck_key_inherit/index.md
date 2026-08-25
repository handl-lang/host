# ac_typecheck_key_inherit

# handl-typecheck-key-inherit

Typechecks the `inheritable` handl key.

Fields with `inheritable: true` propagate their values into nested
objects. This crate checks that override values in nested scopes have
types compatible with the parent declaration.

## Architectural rule

> **Inherited field overrides must be type-compatible with the
> parent declaration.**

## Modules

### [`check`](check.md)

*1 constant, 2 functions*

