**handl_typecheck_core > unify**

# Module: unify

## Contents

**Functions**

- [`check_bounds`](#check_bounds) - Check that all type parameter bounds are satisfied by the substitution.
- [`decompose_type`](#decompose_type) - Decompose a concrete type into a constructor and its type arguments.
- [`substitute`](#substitute) - Apply a substitution to a type, replacing all bound `Var`s with their
- [`types_match`](#types_match) - Check if two types are structurally identical (no unification, no Unknown
- [`unify`](#unify) - Unify `actual` against `pattern`, updating `subst` in place.

**Type Aliases**

- [`Substitution`](#substitution) - A substitution map: type variable name → concrete type.

---

## handl_typecheck_core::unify::Substitution

*Type Alias*: `std::collections::BTreeMap<String, crate::Type>`

A substitution map: type variable name → concrete type.



## handl_typecheck_core::unify::check_bounds

*Function*

Check that all type parameter bounds are satisfied by the substitution.

After unification, each type parameter's bound type (from `TypeParam::bounds`)
is resolved against the substitution and checked against the concrete type
using the provided [`crate::TraitRegistry`].

Returns `true` if all bounds are satisfied (or if bounds are empty).

```rust
fn check_bounds(subst: &Substitution, type_params: &[crate::TypeParam], registry: &crate::TraitRegistry) -> bool
```



## handl_typecheck_core::unify::decompose_type

*Function*

Decompose a concrete type into a constructor and its type arguments.

This is used for HKT unification — e.g., `Array<String>` decomposes into
`(Array, [String])`, and `Object({"a": String})` decomposes into
`(Object, [Object({"a": String})])` (objects are treated as nullary).

Returns `None` for types that cannot be decomposed (primitives, Unknown, etc.).

```rust
fn decompose_type(ty: &crate::Type) -> Option<(crate::Type, Vec<crate::Type>)>
```



## handl_typecheck_core::unify::substitute

*Function*

Apply a substitution to a type, replacing all bound `Var`s with their
concrete types. Unbound `Var`s are left as-is.

```rust
fn substitute(ty: &crate::Type, subst: &Substitution) -> crate::Type
```



## handl_typecheck_core::unify::types_match

*Function*

Check if two types are structurally identical (no unification, no Unknown
wildcard). Used for consistency checks on already-bound variables.

```rust
fn types_match(a: &crate::Type, b: &crate::Type) -> bool
```



## handl_typecheck_core::unify::unify

*Function*

Unify `actual` against `pattern`, updating `subst` in place.

Returns `true` if unification succeeds. On failure, `subst` may contain
partial bindings from before the failure — callers should discard it.

## Rules

- `Unknown` / `Never` in either position: compatible with anything.
- `Var(name)` in **pattern**: bind `name → actual` (or check consistency
  if already bound). This is the primary direction — the pattern is the
  generic signature, `actual` is the concrete argument type.
- `Var(name)` in **actual** but not pattern: only compatible if pattern
  is `Unknown`/`Never` (handled by the catch-all). A concrete pattern
  cannot unify with a variable in the actual position.
- Identical `Var` (same name) in both: compatible.
- Concrete types: structural recursion into `Array`, `Object`, `Function`.

## Type parameter scoping

`type_params` is the set of type parameter declarations that are
considered "in scope" for this unification — only `Var`s whose name
matches a `TypeParam.name` are eligible for binding. A `Var` not in
`type_params` is treated as an opaque type (compatible only with
itself or `Unknown`/`Never`).

## Trait bounds

This function does **not** check trait bounds — it only handles binding.
Use [`check_bounds`] after unification to verify that bound types
satisfy their declared bounds.

```rust
fn unify(actual: &crate::Type, pattern: &crate::Type, subst: & mut Substitution, type_params: &[crate::TypeParam]) -> bool
```



