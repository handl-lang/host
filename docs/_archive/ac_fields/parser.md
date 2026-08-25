**handl_fields > parser**

# Module: parser

## Contents

**Functions**

- [`build_path_expr`](#build_path_expr)
- [`parse_expr_to_tree`](#parse_expr_to_tree) - Parse an expression string into an [`Expr<NonFunctionValue>`] tree.
- [`strip_interpolation`](#strip_interpolation)

---

## handl_fields::parser::build_path_expr

*Function*

```rust
fn build_path_expr(s: &str, dialect: &dyn Dialect, namespaces: &crate::types::Namespaces) -> handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>
```



## handl_fields::parser::parse_expr_to_tree

*Function*

Parse an expression string into an [`Expr<NonFunctionValue>`] tree.

The resulting tree is evaluated by the VM, so path lookups are
encoded as host calls (`var` and `resolve`) and can be resolved
lazily against the current [`Context`].

```rust
fn parse_expr_to_tree(s: &str, dialect: &dyn Dialect, namespaces: &crate::types::Namespaces) -> Option<handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>>
```



## handl_fields::parser::strip_interpolation

*Function*

```rust
fn strip_interpolation<'a>(s: &'a str, dialect: &dyn Dialect) -> &'a str
```



