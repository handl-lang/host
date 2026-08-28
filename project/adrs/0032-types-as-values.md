# Types are values, runtime and typechecker share one value space

## Context

The handl language is converging on a design where the type language and the expression language are the same. Types are not special syntax; they are ordinary handl values. This is the Agda-style "types are values" model.

Current state:

- `handl-vm-type` already has a pure `Value` type with `VFun`, `VNeutral`, `VType`, `VLit`, etc.
- The `.hnl` runtime (`handl-vm-values-json`) only has JSON values: `null`, `bool`, `number`, `string`, `array`, `object`. It has no first-class function values.
- Because of that split, a type like `type.number = (x) -> type.type_of(x) == "number"` cannot exist as a runtime value, even though it is natural in the typechecker.
- The typed examples and docs now assume this unified model, but it cannot run yet.

## Decision

1. Types are ordinary handl values.
2. A valid type is a **predicate**: a value that can be applied to another value and returns `type.bool`.
3. `is` is type application: `x is T` means `T(x)`.
4. `:` is a typed binding: `name: T = e` means `let name = e in (name is T); name`.
5. The runtime and the typechecker will share the same `Value` representation.
6. `handl-vm-type::Value` is the canonical value type for both pure type-level evaluation and effectful runtime evaluation.
7. Static type expressions must be evaluable in a **pure** context: no effects, no IO, no runtime-only values.

## Consequences

- Base type predicates (`type.string`, `type.number`, `type.bool`, `type.null`, `type.array`, `type.object`, etc.) are ordinary functions.
- Record types are objects of predicates: `Person = { name: type.string, age: type.number }`.
- Array-of-T is `array.every(arr, T)`, not a special type combinator.
- Refinement types are ordinary functions with typed parameters: `Even = (n: type.number) -> n % 2 == 0`.
- Function return types are part of the function signature: `addOne = (n: type.number): type.number -> n + 1`.
- The `type` module is tiny. It only needs primitive predicates and `type.type_of`. Everything else is ordinary handl code.
- `type.unknown` remains because it is a value exposed by the typecheck host to signal unknown types to the typechecker. It is not a runtime predicate.
- Function values must carry declared parameter/return type metadata so that `f is NumberToNumber` can be checked. Runtime behavior cannot be observed statically.
- `handl-vm-values-json` will be replaced or become a thin serialization view of `handl-vm-type::Value`.
- The `mood` module becomes redundant. Its runtime behavior can move into the `type` module or become ordinary handl functions.
- The parser/AST needs to support `:` typed bindings and `is` as application.

## Status

proposed
