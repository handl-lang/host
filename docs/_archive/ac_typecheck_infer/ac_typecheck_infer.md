**handl_typecheck_infer**

# Module: handl_typecheck_infer

## Contents

**Modules**

- [`parser`](#parser)
- [`table`](#table)

**Structs**

- [`InferenceWithTypeArgs`](#inferencewithtypeargs) - Result of type inference with resolved type arguments.

**Functions**

- [`check_call`](#check_call) - Check call arguments against a function type, returning the return type
- [`check_call_detailed`](#check_call_detailed) - Like [`check_call`] but returns detailed error information on mismatch.
- [`check_call_with_expected`](#check_call_with_expected) - Like [`check_call`] but also accepts an optional expected return type.
- [`check_call_with_type_args`](#check_call_with_type_args) - Like [`check_call_with_expected`] but also returns resolved type
- [`infer`](#infer) - Infer the type of an expression string.
- [`infer_ast`](#infer_ast) - Infer the type of a parsed expression AST.
- [`infer_ast_detailed`](#infer_ast_detailed) - Like [`infer_ast`] but also collects type errors from function calls.
- [`infer_ast_inner_detailed`](#infer_ast_inner_detailed)
- [`infer_ast_with_expected`](#infer_ast_with_expected) - Like [`infer_ast`] but accepts an optional expected type for the expression.
- [`infer_ast_with_type_args`](#infer_ast_with_type_args) - Like [`infer_with_type_args`] but operates on a parsed [`ExprAst`].
- [`infer_cached`](#infer_cached) - Infer the type of an expression string, with AST caching.
- [`infer_cached_detailed`](#infer_cached_detailed) - Like [`infer_cached`] but also collects type errors from function calls.
- [`infer_with_type_args`](#infer_with_type_args) - Infer the type of an expression string, also resolving type arguments
- [`resolve_path_type`](#resolve_path_type) - Resolve a dotted path string to a [`Type`] via the [`TypeEnv`].
- [`walk_path`](#walk_path) - Walk a dotted path into a type, returning the type at the end.

**Constants**

- [`DEFAULT_NAMESPACE`](#default_namespace) - The conventional name of the default namespace used for shorthand

**Type Aliases**

- [`ParseCache`](#parsecache) - A parse cache mapping expression strings to their parsed ASTs.

---

## handl_typecheck_infer::DEFAULT_NAMESPACE

*Constant*: `&str`

The conventional name of the default namespace used for shorthand
path resolution in type inference (bare `name` → `DEFAULT_NAMESPACE.name`).

This is a runtime host convention, not a language-level requirement.
It mirrors `handl_fields::DEFAULT_NAMESPACE` — kept as a separate constant
because `handl-typecheck-infer` does not depend on `handl-fields`.



## handl_typecheck_infer::InferenceWithTypeArgs

*Struct*

Result of type inference with resolved type arguments.

Contains the inferred type and, for generic call sites, the resolved
type argument strings (one per type parameter, in declaration order).
Each string is the `Type::name()` of the concrete type bound to that
parameter (e.g. `"string"`, `"number"`, `"array<number>"`).

**Fields:**
- `return_type: handl_typecheck_core::Type` - The inferred return type of the expression.
- `type_args: Vec<String>` - Resolved type argument strings for the top-level call, if it was

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> InferenceWithTypeArgs`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typecheck_infer::ParseCache

*Type Alias*: `std::collections::HashMap<String, Option<std::rc::Rc<ExprAst>>>`

A parse cache mapping expression strings to their parsed ASTs.

Uses [`Rc`] so that cache hits are cheap (refcount bump) instead of
deep-cloning the entire AST tree.



## handl_typecheck_infer::check_call

*Function*

Check call arguments against a function type, returning the return type
or [`Type::Unknown`] if args don't match.

For monomorphic functions (`type_params` empty), this is a structural
compatibility check — same as before.

For generic functions (`type_params` non-empty), this unifies argument
types against parameter types to build a substitution, then applies it
to the return type. If unification fails, returns [`Type::Unknown`].

```rust
fn check_call(fn_type: &handl_typecheck_core::FunctionType, arg_types: &[handl_typecheck_core::Type]) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::check_call_detailed

*Function*

Like [`check_call`] but returns detailed error information on mismatch.

Returns `Ok(Type)` on success, or `Err(CallCheckError)` with a description
of what went wrong (wrong arg count, type mismatch, or unification failure).

```rust
fn check_call_detailed(fn_name: &str, fn_type: &handl_typecheck_core::FunctionType, arg_types: &[handl_typecheck_core::Type]) -> Result<handl_typecheck_core::Type, handl_typecheck_core::CallCheckError>
```



## handl_typecheck_infer::check_call_with_expected

*Function*

Like [`check_call`] but also accepts an optional expected return type.

When `expected_return` is provided and the function is generic, the
expected return type is unified against the declared return type
*after* argument unification. This allows type variables that appear
only in the return type (not in any parameter) to be bound from the
context — this is **return-type-driven inference**.

For example, given `make_array<T>() -> Array<T>` called in a context
expecting `Array<String>`, the expected type `Array<String>` is unified
against `Array<T>`, binding `T = String`.

```rust
fn check_call_with_expected(fn_type: &handl_typecheck_core::FunctionType, arg_types: &[handl_typecheck_core::Type], expected_return: Option<&handl_typecheck_core::Type>) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::check_call_with_type_args

*Function*

Like [`check_call_with_expected`] but also returns resolved type
argument strings for generic calls.

Returns `(return_type, type_args)` where `type_args` is a vector of
`Type::name()` strings, one per type parameter in declaration order.
For monomorphic calls, `type_args` is empty.

```rust
fn check_call_with_type_args(fn_type: &handl_typecheck_core::FunctionType, arg_types: &[handl_typecheck_core::Type], expected_return: Option<&handl_typecheck_core::Type>) -> (handl_typecheck_core::Type, Vec<String>)
```



## handl_typecheck_infer::infer

*Function*

Infer the type of an expression string.

Parses the expression, then walks the AST to determine the result type.
Returns [`Type::Unknown`] if the type cannot be determined (e.g., an
unknown function or an unresolvable path).

```rust
fn infer(expr: &str, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::infer_ast

*Function*

Infer the type of a parsed expression AST.

```rust
fn infer_ast(ast: &ExprAst, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::infer_ast_detailed

*Function*

Like [`infer_ast`] but also collects type errors from function calls.

Returns `(Type, Vec<TypeError>)` — the inferred type and any call errors
found in the expression tree. The inferred type may still be `Unknown`
even when errors are present (e.g., a nested call failed but the outer
expression continues).

```rust
fn infer_ast_detailed(ast: &ExprAst, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect) -> (handl_typecheck_core::Type, Vec<handl_typecheck_core::TypeError>)
```



## handl_typecheck_infer::infer_ast_inner_detailed

*Function*

```rust
fn infer_ast_inner_detailed(ast: &ExprAst, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect, errors: & mut Vec<handl_typecheck_core::TypeError>) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::infer_ast_with_expected

*Function*

Like [`infer_ast`] but accepts an optional expected type for the expression.

When the expression is a function call and the expected type is known,
this enables return-type-driven inference: the expected type is passed
to [`check_call_with_expected`] so that type variables appearing only in
the return type can be bound from context.

```rust
fn infer_ast_with_expected(ast: &ExprAst, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect, expected: Option<&handl_typecheck_core::Type>) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::infer_ast_with_type_args

*Function*

Like [`infer_with_type_args`] but operates on a parsed [`ExprAst`].

```rust
fn infer_ast_with_type_args(ast: &ExprAst, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect) -> InferenceWithTypeArgs
```



## handl_typecheck_infer::infer_cached

*Function*

Infer the type of an expression string, with AST caching.

Uses `cache` to avoid re-parsing the same expression string.
The cache maps expression strings to their parsed [`ExprAst`] (wrapped
in [`Rc`] so cache hits are cheap). Pass the same `cache` across
multiple `infer_cached` calls to eliminate redundant parsing.

```rust
fn infer_cached(expr: &str, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, cache: & mut ParseCache, dialect: &dyn Dialect) -> handl_typecheck_core::Type
```



## handl_typecheck_infer::infer_cached_detailed

*Function*

Like [`infer_cached`] but also collects type errors from function calls.

```rust
fn infer_cached_detailed(expr: &str, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, cache: & mut ParseCache, dialect: &dyn Dialect) -> (handl_typecheck_core::Type, Vec<handl_typecheck_core::TypeError>)
```



## handl_typecheck_infer::infer_with_type_args

*Function*

Infer the type of an expression string, also resolving type arguments
for the top-level call if it is a generic function call.

This is the bridge between the typechecker and the runtime: the
resolved `type_args` can be attached to `Expr::Call` nodes so that
`GenericFunction` host implementations can inspect them at runtime.

```rust
fn infer_with_type_args(expr: &str, env: &handl_typecheck_core::TypeEnv, functions: &FunctionTable, dialect: &dyn Dialect) -> InferenceWithTypeArgs
```



## Module: parser



## handl_typecheck_infer::resolve_path_type

*Function*

Resolve a dotted path string to a [`Type`] via the [`TypeEnv`].

Handles both qualified paths (`doc.name`) and shorthand (`name`
resolves via the default namespace).

```rust
fn resolve_path_type(path: &str, env: &handl_typecheck_core::TypeEnv, dialect: &dyn Dialect) -> handl_typecheck_core::Type
```



## Module: table



## handl_typecheck_infer::walk_path

*Function*

Walk a dotted path into a type, returning the type at the end.

```rust
fn walk_path(ty: &handl_typecheck_core::Type, parts: &[&str]) -> handl_typecheck_core::Type
```



