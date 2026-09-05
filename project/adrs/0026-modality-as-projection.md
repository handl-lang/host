# Modality as Set-Theoretic Projection

## Context

Acolang today has many ad-hoc operations that all address the same underlying pattern: a value `x` is known to inhabit some source type `S`, and the program wants to relate `x` to a smaller kind `K ⊆ S`. The surface operations include:

- `type.is_string(x)` — test whether `x` is a string
- `result.ok` / `result.err` — extract a variant payload
- `unwrap`, `assert`, `or` — strict or soft extraction
- `as?`, `getOr`, `when` — safe projection with defaults

These can be unified by observing that they are not different library functions. They are different **moods** applied to a single primitive: **projection**.

In linguistics, modality expresses the speaker's attitude toward a proposition. Transposed to a programming language, modality expresses the program's attitude toward a projection. The same `project(x, K)` operation can be interpreted as a fact (`indicative`), a question (`interrogative`), an obligation (`normative`), a possibility (`potential`), a complement (`negative`), or a condition (`conditional`).

## Decision

Acolang will expose a **modality system** built on top of a primitive **projection**. Given a value `x : S` and a kind `K ⊆ S`:

```text
project(x, K)
```

extracts the part of `x` that inhabits `K`. The six moods are different ways of using that projection.

### Moods

| mood          | module      | sugar             | semantics                             | result                                               |
| ------------- | ----------- | ----------------- | ------------------------------------- | ---------------------------------------------------- |
| indicative    | `mood.ind`  | `x.K`             | strict projection onto `S ∩ K`        | the projected value, or error if `x ∉ K`             |
| potential     | `mood.pot`  | `x may K`         | soft projection onto `S ∩ K`          | the projected value, or `null` if `x ∉ K`            |
| negative      | `mood.neg`  | `x not K`         | soft projection onto `S − K`          | the complement-projected value, or `null` if `x ∈ K` |
| interrogative | `mood.int`  | `x is K`          | membership test                       | `bool`                                               |
| normative     | `mood.norm` | `x must K`        | strict projection wrapped in `Result` | `result.ok(value)` or `result.err(error)`            |
| conditional   | `mood.cnd`  | `x when K else y` | soft projection with default          | the value, or `y`                                    |

### Projection semantics

`x not K` is **not** Boolean negation. It is a complement projection:

```text
Ok(v) not ok      →  null
Err(e) not ok     →  e              // the err payload

Circle(c) not circle        →  null
Rectangle(r) not circle     →  r    // the rectangle payload
Triangle(t) not circle      →  t    // the triangle payload
```

The Boolean "is not" is still expressed by prefix `not`:

```text
not (x is K)      →  !mood.int(x, K)
```

`x may K` and `x not K` are dual projections:

```text
x may K   →  project?(x, S ∩ K)
x not K   →  project?(x, S − K)
```

### Type algebra

For `x not K` to have a useful static type, the type system should eventually support **set subtraction**:

```text
Shape - Circle  =  Rectangle | Triangle
(String | Int | Float) - String  =  Int | Float
A | B | C | D - (B | D)  =  A | C
```

This makes mood not merely syntactic sugar but a surface for **type algebra**. The runtime projection and the static type should agree in the long term. The runtime implementation can proceed before the type system supports subtraction.

### What this combines

The proposed semantics fuse three operations that are normally separate:

1. **Type-set operation** — `S − K` (or `S ∩ K` for the positive moods)
2. **Runtime membership test** — `x ∈ K ?`
3. **Projection** — `payload(x, S − K)` (or `payload(x, K)`)

For example:

```text
x not K  =
  type:    S − K
  test:    x ∈ K ?
  project: payload(x, S − K) if the test fails, null if it succeeds
```

This is the point where the design goes beyond simple type narrowing. A language like TypeScript can express `Exclude<S, K>` for the type-set operation, but it does not offer a single construct that also performs the runtime membership test and the value projection. The mood syntax binds all three together: the type algebra, the runtime check, and the extraction of the resulting value.

### Phasing

This decision is split into two phases:

1. **Phase 1 — Runtime.** Implement the `mood` module and the infix keyword sugar in the canonical lexer and parser. Values are tagged at runtime (e.g., `Result` variants). The type system does not yet compute `S − K`; the static type of `x not K` can be the source type or `null`.

2. **Phase 2 — Type algebra.** Add union (`|`) and subtraction (`−`) to the type system so that `Shape − Circle` normalizes to `Rectangle | Triangle`, and each mood receives a precise static result type.

For now, only Phase 1 is in scope.

### Keywords

The infix mood sugar reserves these keywords:

- `is` — `mood.int`
- `must` — `mood.norm`
- `may` — `mood.pot`
- `not` — `mood.neg` when infix, logical negation when prefix
- `when` / `else` — `mood.cnd`

The indicative mood `x.K` uses the existing dot projection operator and requires no new keyword.

### Phase 1 implementation

#### Lexer

Add the following keywords to `handl-lexer-canonical`:

- `is`, `must`, `may`, `when`, `else`
- `not` (used both as the infix negative mood and as prefix logical negation)

No new punctuation tokens are required. The indicative mood uses the existing `Dot` token.

#### Parser

Introduce a `mood-expr` grammar rule:

```text
mood-expr := primary-expr mood-op kind-path ("else" expr)?
mood-op   := "is" | "must" | "may" | "not" | "when"
kind-path := identifier ("." identifier)*
```

- `else` is required only for `when`.
- `not` is parsed as the infix negative mood when it follows a primary expression; as prefix logical negation when it begins a factor.
- Precedence: mood infix binds after dot access and before binary operators.

#### Runtime

`handl-lib-utils` will provide a `mood` module with the following functions:

| function                         | meaning                                         |
| -------------------------------- | ----------------------------------------------- |
| `mood.ind(value, kind)`          | strict projection; error if `value ∉ kind`      |
| `mood.int(value, kind)`          | membership test; `bool`                         |
| `mood.norm(value, kind)`         | strict projection wrapped in `Result`           |
| `mood.pot(value, kind)`          | soft projection; `null` if `value ∉ kind`       |
| `mood.neg(value, kind)`          | complement projection; `null` if `value ∈ kind` |
| `mood.cnd(value, kind, default)` | soft projection with fallback                   |

The `kind` argument is a runtime kind descriptor. In Phase 1 it can be:

- a variant tag (e.g., `"ok"` or `"err"`)
- a type tag (e.g., `"string"`)
- a field name (e.g., `"name"`)

The canonical infix sugar desugars directly into these calls. For example:

```text
result is ok           →  mood.int(result, "ok")
result must ok         →  mood.norm(result, "ok")
result may ok          →  mood.pot(result, "ok")
result not ok          →  mood.neg(result, "ok")
result when ok else y  →  mood.cnd(result, "ok", y)
result.ok              →  mood.ind(result, "ok")
```

For `Result`, the runtime should represent a result value as a tagged value with either an `ok` or `err` key. `mood` matches the tag and projects the payload.

## Consequences

- The `is_*` family, `unwrap`, `assert`, `or`, and safe-cast functions are replaced by a single, consistent grammar.
- Phase 1 requires a runtime value model with variants and tags (e.g., `Result`, sum types) so that `K` can be matched.
- Phase 1 can proceed without set subtraction in the type system. The static result type of `x not K` may be the source type or `null` until Phase 2.
- Phase 2 will add union (`|`) and subtraction (`-`) to the type system and compute precise result types for every mood.
- The `mood` module becomes the canonical desugaring target for all six moods.
- `not` acquires two distinct but contextually disambiguated roles: infix complement projection and prefix logical negation.
- Fields do not have a natural set-theoretic complement, so `x not field` either requires a separate definition (e.g., "object without the field") or is not supported by the negative mood.

## Status

`proposed`
