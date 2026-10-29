---
title: Effect resumption semantics — `perform`/`handle` as algebra homomorphisms on free arrow-algebras
type: adr
date: 2026-10-25
status: proposed
status_note: Proposed — one-shot in-flight dispatch landed; multi-shot `resume` landed via replay journals on all three engines (`design/multishot-resume.md`)
refs: [ADR-0153, ADR-0154]
---

# ADR-0158: Effect resumption semantics — `perform`/`handle` as algebra homomorphisms on free arrow-algebras

**Refs**: ADR-0153 (dimensions/`~[n]~>`), ADR-0154 (typecheck semantics), Sanada — *Algebraic effects and handlers for arrows* (ICFP/JFP 2024); `design/semantics.md` — the language-wide denotational reading this ADR's correspondence table anchors

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

## One decl: `X := { gen : … ~> result }` — `data ::` removed, `IO` internal

**Generator tables are records.** A `X := { field : type }` record where
fields' types reference the bound name is a former declaration; the
field's codomain shape decides the reading:

| field codomain | the field IS | fold |
|---|---|---|
| `X<…>` (the former) | a `data` ctor | `match`/`X.rec` |
| own `B` (doesn't return `X`) | an `IO` op (suspends) | `handle` |

```hnl
-- data (was: data Nat :: Zero | Succ(pred : Nat)):
Nat := { Zero : Nat, Succ : Nat ~> Nat }

-- indexed (was: data Vec(n : Nat, a : Type) :: VNil | VCons(...)):
Vec<n, a> := {
  VNil  : Vec<0, a>,
  VCons : (k : Nat, x : a, xs : Vec<k, a>) ~> Vec<Succ k, a>
}
-- <n,a> on the LHS = family telescope (the existing implicit-binder pos)

-- effects — the record IS the sig:
State := { get : Unit ~> Nat, put : Nat ~> Unit }
counter : Unit ~> IO<State>        -- or ~> State if the marker drops
```

**`data X :: C | D` is removed** — the record form covers it including
dependent ctors (field names live in the arrow domain). The `::`/`|`
list form was `data`-flavored sugar that implied ctors-returning-the-
former — the record literal *is* the generator table, so `:=` is the
honest spelling. Migration: `.hnl` sources rewrite `data X :: C(args)`
→ `X := { C : (args) ~> X }` — mechanical, `types.yaml`/`desugar` reads
the same telescope.

**`IO` becomes an internal marker** — `IO<S>` marks "read `S`'s fields
as suspensions" at the type level (vs `~> S` reading the record). May
shrink to implementation detail if `handle`-position carries the
interpretation; the user-facing decl is just `S := {…}`.

The `data`/`effect` decls are **one mechanism** — "these are the
generators"; the only difference is what a generator produces:

- codomain `X<…>` → a *value* node (`{__ctor__}`); `match`/`X.rec`
  folds a finished spine.
- codomain `B` (not `X`) → a *suspension* node (`{op, arg, next}`);
  `handle` folds the spine in-flight.

`match` on a finished value and `handle` on a live spine are the same
eliminator read at different moments; the full collapse (`handle`
literally *is* `F.rec` on the op-spine) completes when `next`/resumption
is a first-class value — the multi-shot `resume` workstream.

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

- **Multi-shot resumption by replay, not stack capture.** A clause may
  take a second parameter — the resumption `(payload, resume) ~> …` —
  and `resume` is a first-class, copyable value: nondeterminism
  (`choose`), generators (`yield`). Rather than capturing the
  continuation (costly on bytecode/cranelift), `resume(x)` **re-runs the
  handled body** from the `handle` boundary under a replay journal seeded
  with `entries[..i] ++ [(op, x)]`. See `design/multishot-resume.md`.
  Consequence: handled bodies must be deterministic in their perform
  sequence (a mismatch names the op/index); non-op side effects in the
  prefix re-execute per `resume`.
- **One-shot stays the default.** A 1-ary clause `op: x ~> e` keeps the
  direct-style reading (`resume` = the call returning); no journal is
  installed, so the cost is unchanged.
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

- One-shot continuations only (see above; multi-shot is delivered by
  replay, not continuation capture).
- Shallow handlers (deep-carry covers; shallow adds no expressiveness).
- `extern` FFI-op decl form — separate phase-4 item.
