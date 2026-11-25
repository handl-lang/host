---
title: Unified `new` registry — Inductive / Algebra / Constraint
 type: design
 date: 2026-11-25
 refs: [ADR-0106, ADR-0152, pl-0002, pl-0003]
 status: approved (option B; option C rejected)
---

# Unified `new` registry

Approved direction (user, 2026-11-25): option **B** — one declaration
machinery for all checker registries. `instance` decls, mode algebras,
and inductives become three *schemas* over the same `new` +
`instance` substrate. Option C (handler-based resolution, ADR-0197
direction) was shown and rejected: it loses typeclass ergonomics,
conflicts with call-head mode-algebra selection (which *requires* a
global registry), and dissolves coherence guarantees.

## 1. Today: three syntaxes, one mechanism

```hnl
Nat  := new Inductive { zero : Nat, succ : (pred : Nat) ~> Nat }
Cost := new Algebra(Nat) { compose: (a,b) ~> a+b, unit: 0, order: (a,b) ~> a<=b }

Eq<a : Type> := { mkEq : (f : a ~> a ~> Bool) ~> Eq }   -- bare record!
instance Eq(Nat) := { eq: (x, y) ~> x == y }
```

Underneath, `synth_instance` already provides the shared machinery:
head must be a `type.family`, coherence is declaration-time
no-overlap, the dict is checked at the constraint type, and implicit
`<a, ev : Eq(a)>` binders discharge by unification-driven dictionary
search.

## 2. Proposed surface

```hnl
-- Constraint joins the `new` family — same desugar path as
-- Inductive and Algebra:
Eq := new Constraint(Type) {
  evidence : a ~> a ~> Bool        -- dictionary shape; checked at instance sites
}

-- Registration is unchanged — `instance` is already the fill:
instance Eq(Nat) := { eq: (x, y) ~> x == y }

-- Mode algebras ride the same table (M2):
Cost := new Algebra(Nat) { compose: ..., unit: 0, order: ... }
-- internally registers instance Algebra(Cost-head) := {compose,unit,order}
```

Registry kinds and their schemas:

| `new` kind | Registers | Schema fields | Consumers |
|---|---|---|---|
| `Inductive` | ctor family + spec + eliminators | ctors, indices, positivity | `match`, `F.rec`, spec values (pl-0003) |
| `Algebra` | mode algebra instance | `compose`, `unit`, `order` | `mode_compose`, subsumption |
| `Constraint` | evidence dictionary shape | constraint sig + dict shape | `synth_instance` search |

One `new` desugar → `data` family + spec record + `instance`
registration; `synth_instance` serves all three lookup paths
(evidence binders, call-head mode selection, ctor dispatch).

## 3. Binder modernization (bundled)

The legacy `<a, ev : Eq(a)>` implicit binder gets a typed-binder
equivalent in the same pass — implicit flag on `(ev : Eq(a))` or an
explicit-evidence call form. Decided at implementation; the constraint
mechanism itself is unchanged.

## 4. Non-goals

- No handler-based resolution (option C, rejected).
- No ordered/first-match instance priority (still deferred; coherence
  stays declaration-time no-overlap).
- `Eq<a : Type>` record-spelling families keep working during
  transition (desugar compat, same as `data` migration).

## 5. Sequencing

Depends on `new` declaration syntax landing (agent `b2ef7ae0`,
in flight). Implementation workstream: `new Constraint` kind +
dict-shape checking + `instance` head generalization (currently
requires `type.family` head — Constraint heads must qualify) +
binder modernization + migration of in-tree `Eq`-style families.
