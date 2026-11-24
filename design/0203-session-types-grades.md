---
title: Session Types as Grade Morphisms — design detail
type: design
date: 2026-09-09
updated: 2026-10-29
refs: [ADR-0203]
---

> Imported from `kit-data-prelude/project/design/0102-session-types-grades.md` — ADR-0102 renumbered to ADR-0203 in the host merge.

# ADR-0203: Session Types as Grade Morphisms — design detail

Moved verbatim from ADR-0203 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

## Decision

### 1. Introduce `grade Session`

Add a first-class grade family `Session` with three protocol constructors:

- `!T` — send a value of type `T`.
- `?T` — receive a value of type `T`.
- `End` — close the channel.

These are the objects of the `Session` grade. Each session object denotes a single protocol action. A full protocol is a grade expression built from these constructors.

### 2. Duality

Duality `dual : Session -> Session` is a grade operation and a grade law. It flips the direction of every action:

- `dual(!T)` = `?T`
- `dual(?T)` = `!T`
- `dual(End)` = `End`

It is an involution, meaning applying it twice returns the original session:

```handl
law duality : dual(dual(S)) => S
```

### 3. Sequential composition

Sequential composition of sessions is grade multiplication. The existing `;` or `>>` syntax for grade composition is reused for chaining actions:

```handl
Session(!Int ; ?Bool ; End)
```

This denotes the protocol: send an `Int`, then receive a `Bool`, then close.

### 4. Channel operations

Channel operations are graded arrows whose grade is a `Session` object:

```handl
op send<T> : (Chan, T) ~[Session(!T)]~> Chan
op recv<T> : Chan ~[Session(?T)]~> (Chan, T)
op close : Chan ~[Session(End)]~> Unit
```

- `send<T>` consumes a channel and a value of type `T`, and returns the channel after the send action.
- `recv<T>` consumes a channel and returns the channel along with the received value.
- `close` consumes a channel and returns `Unit`.

### 5. Channel endpoints

A channel is logically a pair of a channel handle and the current `Session` grade. The two endpoints of a channel have dual session grades:

- Client endpoint: `(c, S)`
- Server endpoint: `(c, dual S)`

This way the `send` at one endpoint is the `receive` at the other, and the protocol is consistent at both ends.

### 6. Session types are first-class grades

`Session` is a full grade family. Handlers for `Session` manage the channel state transitions. A `Handle` on a session grade can be used to implement channel runtime behavior by pattern-matching on the current `Session` object and stepping to the next one.

No new `Term` variants are required: the existing `Op`, `Handle`, and `Path` machinery is sufficient to implement `Session` as a grade over the arrow calculus.

## Why

Session types describe bidirectional communication protocols: a channel has a type that says which side may send or receive which values, and in which order, before the channel is closed.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- Session types fit directly into the existing graded arrow calculus without a separate type-theory extension.
- Duality, composition, and grade laws give a compositional way to reason about protocols.
- Channel operations are first-class arrows; they can be composed, handled, and passed around like any other arrow.
- Linear or affine channel management falls out of the grade discipline.
- Deadlock freedom can, in future passes, be enforced by the typechecker using the same grade machinery.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0203-session-types-grades.md`](../design/0203-session-types-grades.md).
