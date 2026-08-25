# ac_typecheck_key_termination

# handl-typecheck-key-termination

Compile-time call-graph analysis to detect non-terminating functions.

This crate builds a static call graph from document-declared function
bodies and detects cycles that would cause infinite recursion at
runtime. Cycles through host builtins (like `add`, `mul`) are safe
because host functions don't call back into user-defined functions.

## Architectural rule

> **A function that unconditionally calls itself (directly or via a
cycle with no host-function leaf) will never terminate.**

## Modules

### [`ac_typecheck_key_termination`](ac_typecheck_key_termination.md)

*2 functions*

