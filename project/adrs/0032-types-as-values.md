# Types are values, runtime and typechecker share one value space

## Context

The handl language is converging on a design where the type language and the expression language are the same. Types are not special syntax; they are ordinary handl values. This is the Agda-style "types are values" model.

Current state:

- `handl-vm-type` already has a pure `Value` type with `VFun`, `VNeutral`, `VType`, `VLit`, etc.
- The `.hnl` runtime (`handl-vm-values-json`) only has JSON values: `null`, `bool`, `number`, `string`, `array`, `object`. It has no first-class function values and no propositional equality.
- Because of that split, a type like `type.number` cannot be a runtime value, even though it is natural in the typechecker.
- The typed examples and docs now assume a unified model, but it cannot run yet.

## Decision

1. A **type** is a collection of values. It can be applied to a value to produce a **membership type** — a collection of proofs. If the membership type is empty, the value is not a member.
2. `is` is **membership evidence**: `x is T` is the type `T(x)`. If `T(x)` is empty, `x` is not in `T`.
3. `:` is a **membership assertion**: `name: T = e` means `let name = e in (e is T); name`.
4. Equality `===` is a **type**, not a boolean. `x === y` has one value `refl` when `x` and `y` are equal, and is empty otherwise.
5. There is **one universe**: `Type`. No separate `Prop` and `Set`. A proposition is a type with at most one inhabitant. A proof is a value.
6. The `type` module provides the base collections: `type.number`, `type.string`, `type.bool`, `type.null`, `type.array`, `type.object`, and `type.eq` (propositional equality). `->` and `{}` are built-in syntax for function and record types.
7. The runtime and the typechecker share the same `Value` representation.
8. Type-level evaluation is **pure, total, and terminating**. Effectful runtime is a separate mode.

## Consequences

- `Even = (n: type.number) -> n % 2 === 0` is a type family. `Even(4)` is `0 === 0`, inhabited by `refl`. `Even(3)` is `1 === 0`, which is empty.
- `Person = { name: type.string, age: type.number }` is a record / sigma type.
- `n: Even = 4` means `n` is `4` and `Even(4)` is inhabited. The proof is erased at runtime.
- `array.every(arr, T)` is a library function that returns a membership type (a proof or an empty type).
- Function return types are part of the function signature: `addOne = (n: type.number): type.number -> n + 1`.
- `f is NumberToNumber` means `NumberToNumber(f)` is inhabited. Function values must carry declared parameter/return type metadata, or the type must be given as a proof.
- `type.any` and `type.unknown` are not in the `type` module. `Any = (x) -> x === x` is ordinary Handl code. `Unknown` is an internal typecheck-host concept, not a runtime predicate.
- `type.type_of` is reflection and out of scope for the core design. It can be added later without changing the model.
- `handl-vm-values-json` will be replaced or become a thin serialization view of `handl-vm-type::Value`.
- The `mood` module becomes redundant. Its runtime behavior can move into the `type` module or become ordinary handl functions.
- The parser/AST needs to support `:` typed bindings, `is` as membership, and `===` as a type.

## Roadmap

1. Universe levels + `data` declarations for inductive types.
2. `rec` / termination checking for the pure type level.
3. Inductive families like `Vec(A, n)`.
4. Dependent pattern matching with a motive.
5. Type inference and unification (`_` holes).
6. Proof erasure at runtime.
7. Tactics (`auto`, `induction`, `rewrite`).

## Status

accepted

## Implementation

Implemented in the active workspace:

- `crates/vm-type` now has the unified `Value`/`Term` space with `Collection`, `Membership`, `Is`, `Eq`, `Refl`, `TypedLet`, `Empty`, primitive base collections (`type.number`, `type.string`, `type.bool`, `type.null`, `type.array`, `type.object`, `type.eq`), and `Null`/`Array`/`Object` runtime values.
- `handl-lib-type` re-exports the primitives and provides `typecheck` and `typecheck_with_prelude`.
- `handl-platform` composes the VM via the new `value` and `vm` modules, re-exporting the unified value and proof system.
- `handl-target-native` re-exports `Value` from the platform.
