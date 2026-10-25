# ADR-0158: Effect resumption semantics — `perform`/`handle` as algebra homomorphisms on free arrow-algebras

**Status**: Proposed
**Refs**: ADR-0153 (dimensions/`~[n]~>`), ADR-0154 (typecheck semantics), Sanada — *Algebraic effects and handlers for arrows* (ICFP/JFP 2024)

## Context

Effects landed as *trace dispatch*: `perform(op, arg)` mints `IO<{op: arg}>` —
a record of ops-performed; `handle` interprets the produced record
entry-by-entry after the fact. That model works but is one semantic step
short of the algebraic model: there is no suspension point, so ops cannot
feed results back into the arrow, and state-threading effects (State)
need chained single-op IO workarounds (dogfood-2 finding).

Sanada's *algebraic effects and handlers for arrows* is the same surface
(ops + handlers on the Lindley–Wadler–Yallop arrow calculus) with
resumption semantics: an op suspends the arrow, the handler's clause
produces the resumption value, execution continues at the op site.

## Decision

**`perform` dispatches in-flight.** `perform(op, arg)` under an enclosing
`handle` resolves the innermost handler frame covering `op`, calls
`h[op](arg)`, and its return value *is* the `perform` result — the arrow
resumes. Uncovered ops mint the residual `IO<{op: arg}>` trace exactly as
today: the trace semantics become the *unhandled* case, not the only one.

**`handle f with h` means**: while `f` executes, `f`'s `perform`s interpret
through `h`; ops `h` doesn't cover remain residual. The type-level sig
`IO<{puts: T}>` is unchanged — it records what the arrow *does*,
independent of who interprets it.

**`handle f with h acc s0`** — stateful handlers via the arrow-calculus
`loop`/trace combinator: `acc` is the `c` wire of
`loop ((args, s) ~> (result, s'))`. Handler impl signature:
`(payload, s) ~> (result, s')`. The accumulator is the trace wire, not a
special frame slot.

**`return` clause** — handlers gain `return: r ~> e` — the algebra's unit
map on the arrow's pure result. Completes the algebra shape.

**Handlers are 2-cells.** A handler maps each op-generator to a real
arrow — a structure-preserving interpretation of the free arrow-algebra
on the signature, i.e. an algebra homomorphism; in the arrow/cell
vocabulary that's a `~[2]~>`-shaped transformation between
interpretations. Handler *laws* are cell equations, checkable with
`~[2]~>`/`~[3]~>` machinery:

```hnl
-- State law: set-then-read returns the set value
law_state : (put(v) >>> get()) ~[2]~> pure(v)
```

## Correspondence (Sanada → handl)

| Sanada | handl |
|---|---|
| Operation `op : A ⇝ B` (signature arrow) | `perform(op, x)` — invokes the `op` generator at `x` |
| Signature Σ | `IO<{ op: A ~> B, … }>` — record-indexed former |
| Free arrow-algebra on Σ | arrow `A ~> IO<Σ>` whose body performs ops |
| Handler = A-algebra homomorphism | `handle f with { op: impl, return: r ~> … }` |
| Algebra carrier / fold state | `acc s` — the `loop` trace wire |
| Clause continuation | the `perform` call site — one-shot (direct style: resume = the call returning) |
| Denotational: strong promonad A in Prof | operational: `handler_stack` frames + `interpret_io_former` |
| 2-cells between interpretations | `~[2]~>`/`~[3]~>` — same machinery |

## Deltas from Sanada (honest)

- **One-shot resumption.** The handler clause returns once — `resume` is
  the call stack. Multi-shot effects (nondeterminism/`choose`,
  generators/`yield` — handler calls `resume` twice) need real
  continuation capture: feasible in tree-walk (closure over remaining
  eval), costly on bytecode/cranelift (stack capture). **Deferred —
  phase-4 workstream with its own design doc.**
- **Operational, not denotational.** The paper's promonad-in-Prof is a
  proof artifact; handl's semantics is the operational interpreter. Same
  laws, different witness.
- **Residual traces kept.** Unhandled ops produce `IO<{op:arg}>` values —
  an inspectable trace the paper's model doesn't have (its ops must be
  handled). Keep: it's the "effects as data" property.

## Consequences

- `runState`/`runWriter`/`runReader` become real `.hnl` combinators over
  `handle … acc` — the dogfood chained-IO workaround dies.
- `perform` without a handler is unchanged (mint residual).
- Deep-carry (`handler_stack`, `handle_dyn`) is unchanged — the frames
  already exist; only *when* dispatch fires moves (at `perform`, not
  post-production).
- `IO`'s sig type is unchanged; coverage checking (WS-F) is unchanged —
  it checks the *type*, which records ops regardless of interpretation.

## Non-goals

- Multi-shot continuations (see above).
- Shallow handlers (deep-carry covers; shallow adds no expressiveness).
- `extern` FFI-op decl form — separate phase-4 item.
