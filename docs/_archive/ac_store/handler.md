**handl_store > handler**

# Module: handler

## Contents

**Functions**

- [`expr_to_json`](#expr_to_json) - Convert an expression to canonical JSON for hashing.
- [`handler_clause_to_json`](#handler_clause_to_json) - Convert a handler clause to canonical JSON for hashing.
- [`hash_expr`](#hash_expr) - Compute the content hash of an expression.
- [`hash_handler_clause`](#hash_handler_clause) - Compute the content hash of a handler clause.
- [`pattern_to_json`](#pattern_to_json) - Convert a pattern to canonical JSON for hashing.

**Type Aliases**

- [`RuntimeExpr`](#runtimeexpr)
- [`RuntimeHandlerClause`](#runtimehandlerclause)

---

## handl_store::handler::RuntimeExpr

*Type Alias*: `handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>`



## handl_store::handler::RuntimeHandlerClause

*Type Alias*: `handl_ast_canonical::HandlerClause<ac_runtime_values::NonFunctionValue>`



## handl_store::handler::expr_to_json

*Function*

Convert an expression to canonical JSON for hashing.

```rust
fn expr_to_json(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> serde_json::Value
```



## handl_store::handler::handler_clause_to_json

*Function*

Convert a handler clause to canonical JSON for hashing.

The JSON structure is:
```json
{
  "op": "fetch",
  "params": ["url"],
  "resume": "k",
  "body": { ... expr json ... }
}
```

```rust
fn handler_clause_to_json(clause: &handl_ast_canonical::HandlerClause<ac_runtime_values::NonFunctionValue>) -> serde_json::Value
```



## handl_store::handler::hash_expr

*Function*

Compute the content hash of an expression.

```rust
fn hash_expr(expr: &handl_ast_canonical::Expr<ac_runtime_values::NonFunctionValue>) -> crate::hash::DefinitionHash
```



## handl_store::handler::hash_handler_clause

*Function*

Compute the content hash of a handler clause.

```rust
fn hash_handler_clause(clause: &handl_ast_canonical::HandlerClause<ac_runtime_values::NonFunctionValue>) -> crate::hash::DefinitionHash
```



## handl_store::handler::pattern_to_json

*Function*

Convert a pattern to canonical JSON for hashing.

```rust
fn pattern_to_json(pattern: &handl_ast_canonical::Pattern) -> serde_json::Value
```



