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
result: type.number = math.abs(-42);
result
```

This expression evaluates to `42`.

## Functions

Functions are always called in `module.function` form:

```handl
item: type.string = array.at(["a", "b", "c"], 1);
item
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
numbers: (a) -> array.every(a, type.number) = [1, 2, 3];
doubled: (a) -> array.every(a, type.number) = array.map(numbers, (n: type.number) -> n * 2);
doubled
```

The final expression is the result of the script.

## Types as values

A type is a **predicate**: an ordinary handl value that you can apply to another value to get a `type.bool`.

```handl
Person: type.object = { name: type.string, age: type.number };
alice: Person = { name: "Alice", age: 30 };
ok: type.bool = alice is Person;
ok
```

`is` is type application: `x is T` means `T(x)`. Record types are just objects of predicates, and array element types are predicates written with `array.every`.

## Refinement types

A refinement type is any function with typed parameters that returns `type.bool`. Because types are values, you can write your own predicates and use them after `:` or `is`:

```handl
Even = (n: type.number) -> n % 2 == 0;
n: Even = 4;
n
```

```handl
Positive = (n: type.number) -> n > 0;
```

```handl
Adult = (p: type.object) -> p.age is type.number and p.age >= 18;
```

Refinements can take parameters, producing a type family:

```handl
DivisibleBy = (k: type.number) -> (n: type.number) -> n % k == 0;

m: DivisibleBy(3) = 9;
m
```

Function return types are written after the parameter list:

```handl
addOne = (n: type.number): type.number -> n + 1;
```
