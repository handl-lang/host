**handl_typecheck_keys**

# Module: handl_typecheck_keys

## Contents

**Modules**

- [`arg`](#arg) - # arg
- [`expr`](#expr) - # expr
- [`inherit`](#inherit) - # inherit
- [`merge`](#merge) - # merge
- [`mutable`](#mutable) - # mutable
- [`termination`](#termination) - # termination

---

## Module: arg

# arg

Typechecks the `arg` handl key.

The `arg` key declares a parameter type for a computed field — the
field's own data value is available as `${arg}` inside its `expr:`.
This module checks that:

- The `arg` value is a valid schema fragment (has a `type` or is an
  object with schema-like structure).
- The `arg` type is compatible with the field's declared type (the
  arg is the field's own value, so it should match).

## Architectural rule

> **The arg type must be compatible with the field's declared type.**



## Module: expr

# expr

Typechecks the `expr` handl key.

For fields with an `expr:` declaration, infers the expression's type
and checks it against the field's declared schema type.

## Architectural rule

> **The inferred type must be compatible with the declared type.**



## Module: inherit

# inherit

Typechecks the `inheritable` handl key.

Fields with `inheritable: true` propagate their values into nested
objects. This module checks that override values in nested scopes have
types compatible with the parent declaration.

## Architectural rule

> **Inherited field overrides must be type-compatible with the
> parent declaration.**



## Module: merge

# merge

Typechecks the `merge` handl key.

Fields with `merge: true` are objects whose properties are spread
into the parent scope at runtime. This module checks that:

- Merge fields have type `object` (merging a non-object is a type error).
- Merged field types are compatible with the parent's declared types
  for the same field names.

## Architectural rule

> **Merge fields must be objects, and their properties must be
> type-compatible with the parent scope.**



## Module: mutable

# mutable

Typechecks the `mutable` handl key.

Fields with `mutable: false` cannot be overridden in nested scopes.
This module walks the data tree and reports any nested overrides of
immutable fields as [`TypeError`]s.

## Architectural rule

> **Immutable fields cannot be overridden in nested scopes.**



## Module: termination

# termination

Compile-time call-graph analysis to detect non-terminating functions.

This module builds a static call graph from document-declared function
bodies and detects cycles that would cause infinite recursion at
runtime. Cycles through host builtins (like `add`, `mul`) are safe
because host functions don't call back into user-defined functions.

## Architectural rule

> **A function that unconditionally calls itself (directly or via a
cycle with no host-function leaf) will never terminate.**



