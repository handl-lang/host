---
title: Tagged Values
type: adr
date: 2026-09-01
status: accepted
---

# ADR-0055: Tagged Values

## Status

Accepted

Refines [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md)
(consumes tagged values for type representation) and [ADR-0052 — Arrow
Calculus Kernel](./0052-arrow-calculus-kernel.md) (`Value` is
`serde_json::Value`). A general host primitive, external to the typechecker.

## Context

ADR-0032 establishes that types are values: the typechecker and runtime share
one value space. This creates a question: how is the type `number`
distinguished from the string `"number"`, if both are values?

The same need arises beyond types — any value-level concept that must stay
distinct from an ordinary value of the same shape:

- A **type** vs the value that denotes its name.
- A **unit of measure** (`Meters(5)` vs `Seconds(5)` — same number, different
  meaning).
- A **capability handle** (`fs` vs the string `"fs"`).
- **Provenance/ownership** (a value marked with where it came from).

This is the same problem JavaScript's `Symbol` solves for keys: `Symbol("number")`
and the string `"number"` never collide. handl needs the analogous property
for values.

### Why not a magic-key convention (rejected)

A prior draft proposed branded values as a convention on JSON: a reserved
`__brand__` key (`{"__brand__":"type.collection","name":"number"}`). This is
rejected for one reason:

**Forgery.** A convention on JSON is forgeable. Any code can write
`{"__brand__":"type.collection",...}`. In a capability-secure language
(ADR-0046), a forgeable tagged value is a hole: user code could construct a
value that claims to be a type the typechecker did not produce. The fix is to
make tagging a **host capability**, not a user-constructible convention. A
value is tagged iff the host minted it; user code cannot forge a tag.

### Why not bare string tags (rejected)

A second draft used bare string tags: `tag("type.pi", payload)`. This is
rejected for one reason:

**Namespace collisions.** String tags can collide across kits. Two kits
that both define `"type.pi"` (or `"unit.meters"`, or `"capability.fs"`)
produce indistinguishable tags even when they mean different things. There is
no way to keep two `type.pi` from different sources distinct, or to assert
that two `type.pi` from the same source are the same, beyond string equality.

### What dependent-type languages do

Agda, Idris, and Lean treat types as first-class values you can pass, compute
with, and return — but the _representation_ of a type value is the
typechecker's, owned internally. User code cannot forge a type value; it can
only produce one by evaluating a type expression through the typechecker's
normalizer. handl adopts the same property, generalized: tagging is a host
capability that mints unforgeable tagged values for any purpose, and tags
carry **registry identity** so collisions across sources are impossible.

## Decision

Introduce **tagged values** as a general host primitive, backed by **tag
registries**. A tagged value is a value carrying a structural tag minted by a
registry. `tag` is a **host capability** (offered by the host, not
user-constructible) — only the host mints tags, so tagged values are
unforgeable. Tags carry **registry identity**, so two tags from different
registries never collide even if their names match. Equality depends on the
registry's semantics (structural or nominal — see §4).

### 1. Tag registries

A **tag registry** is an identity that owns a namespace of tags. There are
two kinds:

- **Host registry**: created by the host at startup. The typechecker's
  `type.*` tags live in a host registry. The host owns its identity; user
  code cannot create or impersonate it.
- **Code-level registry**: created by user code via a host capability
  (`tag_registry()`). Each call to `tag_registry()` mints a fresh, unique
  registry identity. Used for user-defined nominal types, units,
  capabilities, provenance — anything that needs distinctness by identity.

A registry is itself a value (a tagged value minted by the host, e.g.
`{"__tag__":"registry","id":7}`) that can be passed around, stored, and
closed over. Its identity is its minted `id`; two registries are equal iff
they are the same minted registry.

### 2. `tag` / `match_tag` / `tag_registry` — host capabilities

The value language gains three `Arr` ops (ADR-0052), offered by the host as
capabilities:

| Op                             | Meaning                                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| `tag_registry()`               | mint a fresh code-level registry. Host capability — not callable by arbitrary user code unless granted.                   |
| `tag(registry, name, payload)` | mint a tagged value: `{"__tag__": name, "__registry__": registry, …payload}`. Host capability — only the host mints tags. |
| `match_tag(v, cases)`          | dispatch on the tag of `v`; cases bind the payload fields. Available to any code that holds `v`.                          |

`tag` and `tag_registry` are minting capabilities (host-secured);
`match_tag` is the inspection op (anyone holding a tagged value can read its
tag and fields). This matches capability discipline: the power to _create_
a tagged value or registry is restricted; the power to _inspect_ a held value
is not.

### 3. Representation — tagged values are `serde_json::Value`

A tagged value is a JSON object with two reserved keys:

```
{"__tag__": "type.collection", "__registry__": {"id": 0}, "name": "number"}
{"__tag__": "type.pi", "__registry__": {"id": 0}, "param": …, "body": …}
{"__tag__": "meters", "__registry__": {"id": 7}, "value": 5}
{"__tag__": "fs", "__registry__": {"id": 12}, "name": "fs"}
```

`__tag__` is the tag name; `__registry__` is the registry identity that
minted it; the remaining fields are the payload. The value is still a
`serde_json::Value`, so ADR-0052's "no new value type is invented" holds:
tagging is a convention on the existing value space, secured by host
capability and disambiguated by registry identity.

### 4. Two equality flavors, one primitive

The registry determines equality semantics:

- **Structural equality** (host type registry): two tagged values are equal
  iff their `__tag__`, `__registry__`, and payload fields are equal. The
  typechecker's host registry is a single fixed identity, so two
  `type.collection`/`number` tags from it unify by structure — exactly what
  the typechecker needs for base types, Pi/Sigma/Forall.
- **Nominal-by-registry identity** (code-level registries): two tagged
  values are equal iff their `__registry__` identities are the same minted
  registry _and_ their `__tag__` names match. Two `meters` tags from
  _different_ code-level registries are **not equal** even if names and
  payloads match — they are distinct nominal types. Two `meters` tags from
  the _same_ code-level registry are equal. This is what makes user-defined
  types, units, and capabilities distinct by identity.

The typechecker uses the host registry (structural). User code uses code-level
registries (nominal). Both use the same `tag`/`match_tag` primitive; the
registry parameter selects the equality flavor.

### 5. Forgery is impossible

User code cannot construct `{"__tag__":"type.collection",...}` because `tag`
is a host capability. The host mints tagged values; user code receives them
(by evaluating a type expression, calling a unit constructor, etc.) and can
pass them around, but cannot forge one. A user object that happens to have
`__tag__` and `__registry__` keys is not a tagged value — only values minted
by the host `tag` op are. The runtime/typechecker distinguish minted tagged
values from hand-constructed objects by provenance (the host tracks which
values it minted), not by the keys alone.

### 6. Sugar — `type` (and future `unit`, etc.)

Surface sugar applies a tag by name. `type` tags a value as a type:

```
type MyNumber
  ⟶  MyNumber = tag(type_registry(), "type.nominal", { name: "MyNumber" })

type MyNumber = number
  ⟶  MyNumber = number   // alias: MyNumber is the structural type `number`
```

`type X` (no `=`) creates a **nominal** type tagged with its name in the host
type registry — distinct from every other type by name. `type X = T` creates
a **structural alias** — `X` is just `T`. The sugar desugars to `tag` calls
against the host type registry; it introduces no new value representation.
(Future sugar: `unit meters`, `capability fs`, etc., desugar to `tag` calls
against code-level registries minted for that purpose.)

### 7. Untagged values are unchanged

Plain `serde_json::Value`s (strings, numbers, bools, arrays, untagged
objects) are exactly as today. Tagging is opt-in: a value is tagged iff the
host minted it. The runtime and existing eval rules see no difference unless
they call `match_tag`.

## Consequences

### Positive

- **No forgery.** Tagged values are unforgeable: `tag` is a host capability,
  not user-constructible. The capability boundary (ADR-0046) is preserved.
- **No namespace collisions.** Tags carry registry identity; two tags from
  different registries never collide even if their names match. Kits can
  define tags without coordinating string namespaces.
- **Two equality flavors, one primitive.** Structural equality (host type
  registry) for the typechecker's base types and Pi/Sigma/Forall; nominal
  equality (code-level registries) for user-defined types, units,
  capabilities. Same `tag`/`match_tag` primitive; the registry selects the
  flavor.
- **A general primitive.** Tagging serves types, units, capabilities, and
  provenance with one mechanism. Adding a new tagged concept is a new
  registry or tag namespace, not a new value type or engine change.
- **Types are values, without collision.** `number` (the type) is a tagged
  value in the host type registry; `"number"` (the string) is the string.
  The typechecker distinguishes them while keeping both in the shared value
  space — ADR-0032 honored literally, ADR-0054 unblocked.
- **No new value type.** Tagging is a convention on `serde_json::Value`:
  ADR-0052's "no new value type is invented" holds. The eval engine produces
  and consumes tagged values as ordinary JSON objects.
- **Sugar for the common case.** `type X` / `type X = T` desugar to `tag`
  against the host type registry, keeping the surface language readable.

### Negative

- **Reserved keys.** `__tag__` and `__registry__` are reserved at the value
  level. User objects that happen to use them are not tagged values
  (provenance distinguishes them), but the keys are still reserved.
  Mitigation: the keys are double-underscore-prefixed (matching the existing
  `__ast__` convention from ADR-0047); the parser/eval can reject user
  `__tag__`/`__registry__` keys or namespace them.
- **Provenance tracking.** The host must track which values it minted, so a
  hand-constructed `{"__tag__":...}` is not mistaken for a minted tagged
  value. This is a small bookkeeping cost in the host.
- **Registry bookkeeping.** The host must mint and track registry identities
  (host + code-level). This is a small cost; registries are lightweight
  (just an `id`).
- **Two equality flavors.** Structural and nominal equality coexist; rule
  authors and the typechecker must know which a registry uses. Mitigation:
  the host type registry is always structural; code-level registries are
  always nominal. The flavor is determined by the registry, not chosen per
  tag.
- **Tagged values are heavier.** A tagged value is an object with two
  reserved keys, not a bare primitive. Type values that were a single `Name`
  in the old nbe are now a small object. This is the cost of unification;
  acceptable given the typechecker runs at compile time on a small number of
  type values.
- **`match_tag` is a new dispatch op.** Engines (eval, typecheck) must
  implement it. It is a small addition alongside the existing `Arr` ops.

### Scope boundary — what this ADR does NOT promise

- It does **not** introduce nominal-by-reference identity for _objects_.
  Nominal equality is by _registry_ identity, not by value reference; two
  tagged values from the same registry with the same name and payload are
  equal.
- It does **not** define the full set of type tags (`type.collection`,
  `type.pi`, …). That is the ADR-0054 value-encoding spike.
- It does **not** define unit/capability/provenance sugar beyond noting they
  reuse the same primitive with code-level registries. Those are future work.
- It does **not** change the runtime value representation — tagging is a
  convention on `serde_json::Value`, secured by host capability and
  disambiguated by registry identity.

## Status

Proposed — the type tag set and payload field layout are pinned in the
ADR-0054 value-encoding spike. This ADR establishes the primitive (tag
registries + `tag`/`match_tag`/`tag_registry`); ADR-0054 consumes it for
types via the host type registry.
