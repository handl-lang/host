**handl_typecheck_keys > termination**

# Module: termination

## Contents

**Functions**

- [`build_call_graph`](#build_call_graph) - Build a call graph from document-declared function bodies.
- [`check`](#check) - Check all document-declared functions for infinite recursion cycles.
- [`collect_calls`](#collect_calls) - Extract all function call names from an [`ExprAst`] recursively.
- [`deduplicate_cycles`](#deduplicate_cycles) - Normalize a cycle by rotating to start from its lexicographically
- [`detect_cycles`](#detect_cycles) - Detect cycles in a call graph using DFS with three-color marking.
- [`dfs_cycle`](#dfs_cycle)
- [`extract_function_bodies`](#extract_function_bodies) - Extract function bodies from the document data.

**Type Aliases**

- [`CallGraph`](#callgraph) - A call graph mapping function names to the set of functions they call.

---

## handl_typecheck_keys::termination::CallGraph

*Type Alias*: `std::collections::HashMap<String, std::collections::HashSet<String>>`

A call graph mapping function names to the set of functions they call.



## handl_typecheck_keys::termination::build_call_graph

*Function*

Build a call graph from document-declared function bodies.

`function_bodies` is a map of function name → body expression string.
Functions whose body cannot be parsed are skipped (they'll be caught
by the expression typechecker).

```rust
fn build_call_graph(function_bodies: &std::collections::HashMap<String, String>, dialect: &dyn Dialect) -> std::collections::HashMap<String, std::collections::HashSet<String>>
```



## handl_typecheck_keys::termination::check

*Function*

Check all document-declared functions for infinite recursion cycles.

Returns a list of [`TypeError`]s, one per detected cycle.

```rust
fn check(function_bodies: &std::collections::HashMap<String, String>, dialect: &dyn Dialect) -> Vec<handl_typecheck_core::TypeError>
```



## handl_typecheck_keys::termination::collect_calls

*Function*

Extract all function call names from an [`ExprAst`] recursively.

```rust
fn collect_calls(ast: &handl_typecheck_infer::ExprAst, out: & mut std::collections::HashSet<String>)
```



## handl_typecheck_keys::termination::deduplicate_cycles

*Function*

Normalize a cycle by rotating to start from its lexicographically
smallest element, then deduplicate.

```rust
fn deduplicate_cycles(cycles: Vec<Vec<String>>) -> Vec<Vec<String>>
```



## handl_typecheck_keys::termination::detect_cycles

*Function*

Detect cycles in a call graph using DFS with three-color marking.

Returns a list of unique cycles, each represented as a vector of
function names forming the cycle path (e.g. `["foo", "bar", "foo"]`).

```rust
fn detect_cycles(graph: &std::collections::HashMap<String, std::collections::HashSet<String>>) -> Vec<Vec<String>>
```



## handl_typecheck_keys::termination::dfs_cycle

*Function*

```rust
fn dfs_cycle(node: &str, graph: &std::collections::HashMap<String, std::collections::HashSet<String>>, white: & mut std::collections::HashSet<String>, gray: & mut std::collections::HashSet<String>, black: & mut std::collections::HashSet<String>, stack: & mut Vec<String>, cycles: & mut Vec<Vec<String>>)
```



## handl_typecheck_keys::termination::extract_function_bodies

*Function*

Extract function bodies from the document data.

Functions are stored as JSON objects with a `body` string field,
typically under `doc.functions.<name>` or directly as field values
with `type: "function"`.

```rust
fn extract_function_bodies(data: &serde_json::Value) -> std::collections::HashMap<String, String>
```



