---
title: Effects as ambient modes — design
type: design
date: 2026-11-25
revised: 2026-11-26 (v2 — granular effect algebras + algebra interaction)
refs: [ADR-0153, ADR-0158, ADR-0203, ADR-0207, pl-0002, pl-0004]
status: approved-direction v2 (granular effects approved 2026-11-26; `Effects(S)` retired)
---

# Effects as ambient modes

Approved direction (user, 2026-11-25; revised 2026-11-26): effects
migrate from the `IO<sig>` codomain former to the **ambient mode**
reading of MTT — the judgement carries a mode `μ`; `perform` extends
it; `handle` is a modality between modes. The `IO<S>` codomain former
is **retired, not kept as an alias**.

**v2 change (approved 2026-11-26):** effects are *granular* — each
operation is its own `new Algebra` head, not a row inside a single
`Effects` algebra. `Effects(S)` dies. Algebra–algebra interaction is
first-class via a `requires` field (see §4).

This supersedes the "defer unification" verdict of
`effects-as-modes.md` (M4 assessment): that doc correctly identified
the blockers; this doc sequences around them instead of abandoning
the goal.

## 1. The judgement form

MTT's mode context, made concrete for Handl:

```text
Γ ⊢ t : A @ μ        — t has type A in ambient mode μ
μ is a SET of algebra applications (one element per algebra head)
```

Surface spelling:

```hnl
greet : Str @ [PutStr, Cost(3)]
handle m with {puts: h}      -- modality: μ → μ′
```

`@ [..]` replaces *both* reified spellings: the `IO<S>` codomain
former (retired) and the `~[m]~>` graded-arrow mode form (retired —
`~[n]~>` keeps only its dimension meaning). Set semantics: order
irrelevant, one element per head, duplicate heads rejected, `[]` =
pure.

## 2. Granular effect algebras (v2)

Every effect operation is declared as its own algebra. The payload
of a mode element is the operation's *type* `A ~> B` — or a
parameter for parameterized operations:

```hnl
PutStr  := new Algebra { op : Str ~> Unit }
ReadFile := new Algebra { op : Path ~> Str, requires : [FsRead] }
Raise   := new Algebra { op : Err ~> a }        -- parametric codomain
Cost    := new Algebra { compose: (+), unit: 0, order: (<=) }
```

- `op` is the operation signature — `perform` reads domain/codomain
  from it. One shape, no dual declared-vs-payload form.
- Effect algebras need no `compose`/`order` of their own: their
  composition is **set union** (two computations performing `PutStr`
  still perform `PutStr`), and their subsumption is **set inclusion**
  (a context allowing `{PutStr, ReadFile}` satisfies one needing
  `{PutStr}`). The generic mode machinery handles both.
- `requires` (see §4) declares implication between heads.
- `Effects(S)` — the single algebra wrapping a whole signature row —
  is **retired**. A signature `S = {puts: Str ~> Unit}` elaborates to
  the *set* `[PutStr, ...]`, one element per op. There is no row type;
  the row was a reification artifact.

What this deletes from the v1 implementation: `mode_rows_union`'s
per-entry merge rule, `row_width` order, `mode_merge_entry`, the
`Effects` payload-as-record encoding, and the symbolic-row special
cases in `perform`/`handle`. Mode composition becomes uniform
set-union + per-head `compose` on payloads.

## 3. Semantics per construct

| Construct | `IO<sig>` (retired) | Ambient mode (granular) |
|---|---|---|
| `perform(op, arg)` | mints `IO{op:T}` | `PutStr ∈ μ` → check `arg : Str`, return `Unit`; judgement mode unchanged (op already in μ) or extended if `perform` introduces a fresh op |
| composition `>>>` | codomain unification + `io_headed` | set union of mode elements; per-head `compose` on payloads where declared |
| `handle m with h` | `handle_sig_check` | modality `μ → μ′`: coverage = handled heads ⊆ μ; residual `μ′ = (μ ∖ H) ∪ {op ↦ ret} ∪ μ_handler` |
| widening | `IO(s) <: IO(s')` | set inclusion + per-head `order` on payloads |
| row polymorphism | sig metas | mode metavariables (honest bound; M3 posture) |

Residual detail: a handled op does not vanish — it stays in `μ′`
with its payload rewritten to the *return* type (`PutStr ↦ Unit`),
so a deep handler can re-perform it and an outer handler can still
see it. `μ ∖ H` removes only ops the handler fully consumes; the
`op ↦ ret` entries are the residual requests.

`IO<S>` removal stands: `type.io` former retired outright; `@ [..]`
is the only spelling.

## 4. Algebra interaction (v2 addition)

"Algebra between algebras" has three channels, all data-driven:

### 4.1 `requires` — declared implication

```hnl
ReadFile := new Algebra { op : Path ~> Str, requires : [FsRead] }
```

Elaboration closes the mode set under `requires`, transitively:
ascribing `@ [ReadFile]` elaborates to `@ [ReadFile, FsRead]`.
`perform(ReadFile, p)` under `μ = [ReadFile]` implicitly performs
`FsRead` too — the ambient mode must contain the closure.

This is the capability-calculus reading: `requires` is *effect
implication* (Koka's row `:>` includes it implicitly; here it is
declared data on the algebra, checked at elaboration).

### 4.2 Handlers are algebra morphisms μ → μ′

`handle m with h` is literally a homomorphism from the free theory
(the ops in μ) to the residual theory μ′: each handled op maps to
its return type or to the handler's own effect row. A handler that
performs `FsWrite` while handling `PutStr` yields
`μ′ = residual ∪ [FsWrite]` — interaction *between* effects happens
here, at the judgement level, for free.

### 4.3 Cross-head subsumption via `order`

A composite capability algebra can subsume granular ones:

```hnl
Console := new Algebra {
  op : ConsoleSig ~> a,
  order : (big, small) ~> covers(big.ops, small)   -- payload-aware
}
```

`@ [Console(S)]` satisfies a context needing `@ [PutStr]` when
`order` says the console payload covers `puts`. This is optional
grouping sugar over the granular set — never required.

**What is deliberately not built:** a solver that infers
`requires`-closures or merges arbitrary heads at `compose` time.
Shared non-`Effects` heads compose by payload `compose` if declared,
else honest bound (warn + fresh meta) — M3 posture throughout.

## 5. What transfers vs. what must be built

**Transfers:**
- Mode set machinery (M2/M3): `mode_compose` = union + per-head
  compose; `modes_subsumed` = inclusion + per-head `order`.
- `handle_sig_check` coverage/residual logic — re-homed behind the
  modality judgment, re-keyed from row entries to mode elements.
- Runtime untouched: `handler_stack`, `perform` dispatch, replay
  journals are operational.

**Must be built / changed vs. v1 implementation:**
1. Mode contexts `Γ ⊢ t : A @ μ` — done in E1 (judgement slot).
2. `@ [Algebra(payload)]` surface — done in E1.
3. `perform`/`handle` ambient semantics — done in E2 **against the
   `Effects` row encoding; re-key to granular heads** (the judgement
   plumbing stands, the row lookup becomes head lookup).
4. Mode metavariables — done in E3, unchanged.
5. **New in v2:** `requires` field on `new Algebra` + elaboration-time
   closure; per-op `new Algebra` declarations replacing the `Effects`
   algebra; deletion of row-union/merge/row-width special cases.
6. `IO<S>`/`type.io` removal + corpus migration — deprecation
   warnings landed in E1/E4; full removal pending corpus codemod.

## 6. Phases (amends pl-0004)

E0–E4 are implemented on `mtt-modes` against the v1 `Effects`-row
encoding. v2 re-sequences as:

- **G0** — granular `new Algebra` effect declarations: `op` +
  `requires` fields; `@ [PutStr]` bare-head elements; `requires`
  closure at elaboration.
- **G1** — re-key `perform`/`handle`/residual/subsumption from row
  entries to mode elements; delete `Effects` row machinery
  (`mode_rows_union` entry-merge, `row_width`, symbolic-row paths).
- **G2** — corpus migration: `Effects(S)` spellings → granular sets;
  `IO<sig>` sites → `@ [..]`; remove `type.io` former outright.
- **G3** — model parity for G0–G2 in `typecheck_model.hnl`.
- **G4** — e2e verification + merge.

E0–E3 NBE work is *not* wasted: the judgement slot, `@` surface,
modality plumbing, and metavariables all carry over; only the
payload encoding changes.
