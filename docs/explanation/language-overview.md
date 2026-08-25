# Language overview

This page describes the building blocks of handl: values, expressions, functions, and modules.

## Values

handl has six value types, identical to JSON:

- `string` — Unicode text, e.g. `"hello"`
- `number` — integers and floating point, e.g. `42` or `3.14`
- `boolean` — `true` or `false`
- `null`
- `array` — ordered lists, e.g. `[1, 2, 3]`
- `object` — key/value maps, e.g. `{"name": "handl"}`

## Expressions

An expression evaluates to a value. The simplest expression is a literal. More complex expressions call functions:

```handl
math.abs(-42)
```

This expression evaluates to `42`.

## Functions

Functions are always called in `module.function` form:

```handl
array.at(["a", "b", "c"], 1)
```

The first argument is the receiver (often the value being operated on), followed by additional arguments. Conventionally, array and string utilities take the value first.

## Modules

Modules group related functions. Examples:

- `array` — array utilities (`at`, `chunk`, `compact`, `concat`, ...)
- `str` — string utilities (`trim`, `upper`, `split`, ...)
- `math` — numeric utilities (`abs`, `sum`, `avg`, ...)
- `crypto` — hashing (`sha256`, `md5`)
- `fs` — filesystem operations (`read`, `write`, `exists`)

The runtime decides which modules are available and how they are backed. For example, `fs` may be backed by `std::fs` on a server and by an in-memory stub in tests.

## Scoping

A name binding `name = value` is available in the rest of the expression:

```handl
nums = [1, 2, 3]
doubled = array.map(nums, (n) => n * 2)
doubled
```

The final expression is the result of the script.
