# ac_typecheck_key_expr

# handl-typecheck-key-expr

Typechecks the `expr` handl key.

For fields with an `expr:` declaration, infers the expression's type
and checks it against the field's declared schema type.

## Architectural rule

> **The inferred type must be compatible with the declared type.**

## Modules

### [`check`](check.md)

*1 constant, 4 functions*

