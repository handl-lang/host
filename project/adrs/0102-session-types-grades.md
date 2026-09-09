# ADR-0102: Session Types as Grade Morphisms

## Status

Proposed.

## Context

Session types describe bidirectional communication protocols: a channel has a type that says which side may send or receive which values, and in which order, before the channel is closed.

Grades in Handl already classify arrows and propositions. A grade is a resource annotation that can be composed and reasoned about; the arrow calculus in ADR-0101 provides the 2-cell/grade machinery over which these annotations are built.

This ADR proposes representing session types as a `Session` grade whose objects are protocol actions and whose morphisms are dualities and sequential compositions. The idea is to reuse the existing graded arrow calculus rather than inventing a separate type system for channels.

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

## Consequences

### Positive

- Session types fit directly into the existing graded arrow calculus without a separate type-theory extension.
- Duality, composition, and grade laws give a compositional way to reason about protocols.
- Channel operations are first-class arrows; they can be composed, handled, and passed around like any other arrow.
- Linear or affine channel management falls out of the grade discipline.
- Deadlock freedom can, in future passes, be enforced by the typechecker using the same grade machinery.

### Negative

- Requires the grade system to support duality and the `Session` family natively.
- Protocols are fixed at the object level; more advanced session features (branching, recursion, delegation) need follow-up ADRs.
- Runtime channels must track the current session grade, which adds state to the value model.

## Surface syntax examples

```handl
op send<T> : (Chan, T) ~[Session(!T)]~> Chan
op recv<T> : Chan ~[Session(?T)]~> (Chan, T)
op close : Chan ~[Session(End)]~> Unit

law duality : dual(dual(S)) => S
```

A full client/server protocol can be written as a pair of dual graded arrows, e.g. a client with grade `Session(!Int ; ?Bool ; End)` and a server with grade `Session(?Int ; !Bool ; End)`, where the server's session is the dual of the client's.

## References

- ADR-0101 defines the frontier calculus `Term` variants and the 2-cell/grade machinery on which this design is built.
- ADR-0052 defines the arrow-calculus kernel.
