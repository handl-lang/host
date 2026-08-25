**handl_compiler > visitor**

# Module: visitor

## Contents

**Functions**

- [`fold_expr`](#fold_expr) - Fold an expression tree.
- [`fold_expr_inner`](#fold_expr_inner)
- [`fold_field`](#fold_field) - Fold a field.
- [`walk_expr`](#walk_expr) - Walk an expression tree with a visitor.
- [`walk_extracted`](#walk_extracted) - Walk all schemas and data in extracted fields.
- [`walk_field`](#walk_field) - Walk a field with a visitor.

**Traits**

- [`Folder`](#folder) - Transforming IR visitor. Returns new nodes. Override only the
- [`Visitor`](#visitor) - Read-only IR visitor. Override only the methods you care about.

---

## handl_compiler::visitor::Folder

*Trait*

Transforming IR visitor. Returns new nodes. Override only the
methods you care about — defaults recurse and rebuild.

**Methods:**

- `fold_value`
- `fold_expr`
- `fold_field`



## handl_compiler::visitor::Visitor

*Trait*

Read-only IR visitor. Override only the methods you care about.

**Methods:**

- `visit_value`
- `visit_expr`
- `visit_field`
- `visit_extracted`



## handl_compiler::visitor::fold_expr

*Function*

Fold an expression tree.

```rust
fn fold_expr<F>(folder: & mut F, expr: handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>
```



## handl_compiler::visitor::fold_expr_inner

*Function*

```rust
fn fold_expr_inner<F>(folder: & mut F, expr: handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>
```



## handl_compiler::visitor::fold_field

*Function*

Fold a field.

```rust
fn fold_field<F>(folder: & mut F, field: handl_ast_canonical::Field<ac_runtime_values::NonFunctionValue>) -> handl_ast_canonical::Field<ac_runtime_values::NonFunctionValue>
```



## handl_compiler::visitor::walk_expr

*Function*

Walk an expression tree with a visitor.

```rust
fn walk_expr<V>(visitor: & mut V, expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>)
```



## handl_compiler::visitor::walk_extracted

*Function*

Walk all schemas and data in extracted fields.

```rust
fn walk_extracted<V>(visitor: & mut V, extracted: &handl_fields::ExtractedFields)
```



## handl_compiler::visitor::walk_field

*Function*

Walk a field with a visitor.

```rust
fn walk_field<V>(visitor: & mut V, field: &handl_ast_canonical::Field<ac_runtime_values::NonFunctionValue>)
```



