# handl_typecheck_infer

# handl-typecheck-infer

Expression type inference for the handl typechecker.

Parses expression strings (the same syntax used inside `${...}` and
`expr:` fields) and infers their static type given a [`TypeEnv`] and
a [`FunctionTable`].

## Architectural rule

> **Inference is pure — no I/O, no evaluation, no side effects.**

The expression syntax is:
- Literals: `"string"`, `123`, `true`, `false`, `null`
- Path lookups: `doc.name`, `env.HOME`, `name` (shorthand for `doc.name`)
- Function calls: `add(1, 2)`, `mul(x, 2)`, `add(mul(2, 3), 4)`

## Modules

### [`handl_typecheck_infer`](handl_typecheck_infer.md)

*1 constant, 1 struct, 1 type alias, 15 functions, 2 modules*

### [`parser`](parser.md)

*1 enum, 1 struct, 4 functions*

### [`table`](table.md)

*1 type alias, 2 structs*

