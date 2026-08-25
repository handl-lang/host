# ac_typecheck_key_arg

# handl-typecheck-key-arg

Typechecks the `arg` handl key.

The `arg` key declares a parameter type for a computed field — the
field's own data value is available as `${arg}` inside its `expr:`.
This crate checks that:

- The `arg` value is a valid schema fragment (has a `type` or is an
  object with schema-like structure).
- The `arg` type is compatible with the field's declared type (the
  arg is the field's own value, so it should match).

## Architectural rule

> **The arg type must be compatible with the field's declared type.**

## Modules

### [`ac_typecheck_key_arg`](ac_typecheck_key_arg.md)

*1 constant, 4 functions*

