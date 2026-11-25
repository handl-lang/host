---
title: Effects as ambient modes — design
 type: design
 date: 2026-11-25
 refs: [ADR-0153, ADR-0158, ADR-0203, ADR-0207, pl-0002, pl-0004]
 status: approved-direction (phased; research-grade core)
---

# Effects as ambient modes

Approved direction (user, 2026-11-25): effects migrate from the
`IO<sig>` codomain former to the **ambient mode** reading of MTT —
the judgement carries a mode `μ`; `perform` extends it; `handle` is a
modality between modes. `IO<S>` survives as the *reification* of
`{io: S}`, so the existing surface keeps working throughout.

This supersedes the "defer unification" verdict of
`effects-as-modes.md` (M4 assessment): that doc correctly identified
the blockers; this doc sequences around them instead of abandoning
the goal.

## 1. The judgement form

MTT's mode context, made concrete for Handl:

```text
Γ ⊢ t : A @ μ        — t has type A in ambient mode μ
μ ranges over algebra elements (mode values)
```

Surface spelling (proposal, to be pinned by the phase-1 spec):

```hnl
printLn : (Str ~> Str) @ {io: {puts}}
handle m with {puts: h}      -- modality: μ → μ′
```

`@ μ` is a new type-position/judgement suffix — distinct from `~[m]~>`
(which grades the *morphism's usage*) and from `IO<S>` (which is the
*reified* ambient mode, kept for compatibility).

## 2. The effects algebra

```hnl
Effects := new Algebra(Row) {
  compose: (a, b) ~> rowUnion(a, b),   -- shared ops: entries must agree
  unit:    {},
  order:   (a, b) ~> rowSubset(b, a)   -- widening: b ⊇ a
}
```

- `compose` = row union; a shared op keeps its entry iff the two
  entries unify, else fresh meta / honest bound (the merge rule the
  M4 assessment flagged).
- `order` = the existing `dep_unify io_headed` direction, re-homed as
  an algebra field.
- One sig-entry shape going forward: `perform` extends the mode with
  `{op: argTy}` — dissolves the declared-op vs payload dual shape.

## 3. Semantics per construct

| Construct | Today (`IO<sig>`) | Ambient mode |
|---|---|---|
| `perform(op, arg)` | mints `IO{op: T}` value | requires `op ∈ μ`, returns `T`; judgement mode extends to `μ ∪ {op:T}` |
| composition `>>>` | codomain unification + `io_headed` | `mode_compose` on ambient modes |
| `handle m with h` | `handle_sig_check`: coverage, residual `IO(S∖H)∪{op↦ret}` | modality `μ → μ′`: coverage check unchanged (type-directed), residual computed as mode transform |
| widening | `IO(s) <: IO(s')` row-width | `order` field |
| row polymorphism `IO(s)` | sig metas in `Former{type.io}.args` | mode metavariables (honest bound; M3 posture) |

`IO<S>` reification: `IO<S>` elaborates to `@ {io: S} (· ~> ·)`-style
ambient annotation — the codomain former becomes sugar over the mode.
Deep-handler codomain rewrite becomes a modality on the mode slot.

## 4. What transfers vs. what must be built

**Transfers:**
- effects algebra `compose`/`unit`/`order` = row union/∅/width —
  same shapes M2 already ships.
- `mode_compose` at the composition seam computes effect rows for free.
- Runtime untouched: `handler_stack`, in-flight `perform` dispatch,
  replay journals are operational.

**Must be built (the research core):**
1. **Mode contexts** — `Γ ⊢ t : A @ μ`; every synth/check judgment
   gains a mode slot. Largest single change.
2. **`perform` mid-expression mode extension** — context threads μ
   through evaluation order.
3. **Coverage/residual as modality** — `handle_sig_check` logic
   re-homed behind a uniform modality judgment (not deleted — it is
   genuinely type-directed).
4. **Mode metavariables** for row polymorphism — honest bound first,
   solver later.
5. **`IO<S>` ↔ mode reification** — bidirectional elaboration so old
   and new spellings denote one mode.

## 5. Phases (see pl-0004)

- **E0** — effects algebra instance + `~[{io:S}]~>` permitted as
  annotation (M2 subset; no semantic change).
- **E1** — mode contexts: `Γ ⊢ t : A @ μ` judgement plumbing through
  NBE; `@ μ` surface parses/elaborates; `IO<S>` reifies to ambient.
- **E2** — `perform` ambient extension + `handle` modality; coverage/
  residual ported behind the modality judgment.
- **E3** — mode polymorphism over effects (`runAll`-style); mode
  metavariables for row-poly sigs.
- **E4** — model parity: port all of the above into
  `typecheck_model.hnl` (prerequisite debt I-005 lands here or earlier).

E0 is safe to ship inside M2. E1+ is research-grade by design —
explicitly accepted.
