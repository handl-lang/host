---
title: Effects-as-modes assessment (PL-0002 M4)
type: design
date: 2026-11-25
refs: [ADR-0153, ADR-0158, ADR-0203, ADR-0207]
---

# Effects-as-modes — M4 assessment

Assessment only (pl-0002 M4). Question: can Handl's `IO<sig>` /
`handle` / `perform` effect system eventually be expressed as a mode
algebra instance under the MTT framing of `~[m]~>`? **Verdict:
defer unification; `IO<sig>` stays canonical.** An optional cheap
subset is recorded in §5.

## 1. What exists (verified)

- `IO<sig>` is an indexed former: `Former{type.io}` whose sig index is
  an op-name → signature record (`dependent.rs` `dep_synth_call_type`
  IO arm ~3893, `io_sig` ~4237, builtin table ~3616). Spellings:
  `IO<{puts: T}>`, `IO<puts, fs>` sugar, `IO()` = `IO(⊤)`, bare `IO`,
  and row-polymorphic `IO(s)` via the `Sig` sort (`fresh_row_meta`,
  sig metas inside `Former{type.io}.args`, pending-instance discharge).
- `perform(op, arg)` — `synth_perform` (`dependent.rs` ~9717) mints
  `IO{op: T}`; `is_perform_call` (~9663) intercepts inside `synth_call`
  / `check_call_judgment` because the op operand is a *name*, not a
  checkable value.
- `handle` — `synth_handle` / `handle_result_type` /
  `handle_sig_check` (`dependent.rs` ~10272–10480): coverage check
  against the handler record's fields, spurious-impl and
  covers-nothing diagnostics, residual `IO(S')` =
  `(S ∖ S_h) ∪ {op ↦ impl_ret_ty}`, honest-bound passthrough on
  symbolic sigs / non-record handlers, deep-handler rewrite of
  `A ~> IO(S)` codomains (passthrough on `type.pi`).
- Sig subtyping lives in `dep_unify`'s `io_headed` arm (~6160):
  `IO(s) <: IO(s')` iff `s'` names at least `s`'s ops — record-width,
  the "widening" direction (`IO{puts}` usable as `IO{puts,fs}`).
- Runtime is operational and untouched by any type-level
  re-representation: `handler_stack` deep-carry frames, in-flight
  `perform` dispatch (ADR-0158), replay-journal multi-shot resume
  (`design/multishot-resume.md`).
- **Model gap (side finding)**: `synth_handle`, `synth_perform`, and
  `check_call`'s perform branch exist **only as NBE-native judgment
  ops** (`tool-typecheck-nbe/src/lib.rs` ~2140 dispatch). The
  `.hnl` model's judgment-op table (`typecheck_model.hnl` ~5564–7252)
  has no `synth_handle`/`synth_perform` — `type.handle` falls through
  to the "unknown judgment op" arm (~7252). Effects *checking* is
  currently NBE-only Rust; the canonical model lags ~800 lines of
  dependent.rs logic (meta solving, `domain_lift`/`domain_quote`,
  `check_subtype`, diagnostics). Any effects-adjacent work — M4
  included — first owes that port.

## 2. The algebra question

Effects-as-modes minimally needs a mode theory over effect rows
`Σ = {op: sig}`:

| Operation | Effect-row reading | Status vs M2 interface |
|---|---|---|
| compose `⊕` | row union; shared ops need an entry merge | same *shape* as the planned record-row-union default algebra, but sig entries have two shapes (`op : A ~> B` declared sigs vs `perform`'s payload type `T`) — merge rule needed (equal → keep; else fresh meta / honest bound) |
| unit | empty row `∅` (pure) | trivial |
| order `≤` | row-width subsumption: `s₁ ≤ s₂` iff `s₂ ⊇ s₁`'s ops | literally the existing `dep_unify` `io_headed` direction — expressible as the algebra's `order` field |
| handler depth / coverage | `S' = (S ∖ S_h) ∪ {op ↦ ret_ty}` | **not an algebra operation** — set difference + per-entry replacement driven by the handler impl record's *types*, not by the row. MTT vocabulary: a modality / 2-cell between modes (ADR-0158's algebra homomorphism), not a 1-cell composition. The compose/unit/order interface has no slot for it |

So the row half of effects fits the M2 algebra interface almost for
free; the *handler* half does not fit at all — and the handler half is
where all the checking value lives.

## 3. Representation change or semantic change?

ADR-0158 already supplies the categorical story: the sig record *is*
the signature Σ of generators; an effectful arrow body is the free
arrow-algebra on Σ; a handler is an algebra homomorphism (a 2-cell /
modality in MTT terms). Under full MTT the natural reading is "effect
context `E` is a mode; judgements carry `@ E`; `handle` is a modality
`E → E'`". That story is *compatible* with `~[m]~>` — but handl's
implementation deliberately chose the **Koka-style codomain-former**
reading instead: the row lives in `IO(S)`'s type index, not on the
morphism. Two consequences:

- **Modes-on-arrows would create a second source of truth.**
  `A ~[{io: S}]~> IO<S'>` can disagree with itself; either
  `synth_graded_arrow` cross-validates mode-vs-codomain (cheap, one
  row comparison) or users get unchecked parallel annotations —
  strictly worse than today.
- **The ambient-mode reading requires exactly the machinery honest
  scoping excludes**: mode contexts (`Γ ⊢ M : A @ m`), mode
  substitution, mode polymorphism. `perform` minting `IO{op:T}`
  mid-expression works today *because* the effect is a value in the
  codomain type flowing through ordinary unification; an ambient-mode
  `perform` must instead *modify the judgement's mode* — no such
  context exists.
- **Residual traces are a feature, not an accident.** ADR-0158 keeps
  `IO<{op:arg}>` as inspectable data an outer handler or user can
  destructure. An ambient mode on the judgement has no residual
  *value*; preserving the property means reifying the mode as a type —
  i.e., keeping `IO<S>` as the mode's reification, which is a rename
  with extra steps unless the full MTT context machinery also lands.

Net: the *categorical* story exists; the *implemented* story is a
different (codomain-index) representation whose checking machinery —
coverage, residuals, row metas, widening — is type-directed and has no
mode-algebra analogue short of rebuilding it behind a poorer
interface.

## 4. What transfers vs. what doesn't

**Transfers cheaply:**

- The sig record as a mode *value* — records are already arbitrary
  Handl data; `~[{io: {puts, fs}}]~>` parses today.
- Row union / width order as algebra `compose`/`order` fields — same
  shapes M2 already plans; the `io_headed` subtyping direction is the
  order.
- Runtime — `handler_stack`, in-flight dispatch, replay journals are
  operational; untouched.

**Does not transfer (concrete blockers):**

1. **Coverage/residual computation** (`handle_sig_check`): needs
   handler impl types, `check_subtype(req, dom)`, impl-return
   extraction, three diagnostics classes. Type-directed; expressing it
   as "the algebra's handle op" relocates identical code behind a data
   interface and gains nothing.
2. **Row polymorphism**: `IO(s)` sig metas, `Sig` binders,
   `Former{type.io}.args` quoted sigs, pending-instance discharge are
   unification machinery on a type index. Modes have no metavar story;
   M3 already passes on unresolvable modes, so effects would *lose*
   solving that works today.
3. **Deep-handler codomain rewrite**: `handle` rewrites the `IO` former
   inside `type.function`/`type.pi` codomains. The mode analogue would
   rewrite a mode slot that `type.function` doesn't have — the
   checker's function-type representation carries no grade/mode field
   today (`Arrow.grade` lives on signature records, not on
   `type.function` values).
4. **Variance collision**: `IO{puts} <: IO{puts,fs}` widens *outward*
   (doing less is usable where more is declared) — opposite intuition
   from coeffect consumption. One `order` field per algebra absorbs
   it, but it documents that an "effects mode" and a "usage mode" are
   different readings even when both payloads are rows.
5. **Model parity gap** (§1): effects checking is NBE-Rust-only; the
   `.hnl` model — canonical per the thin-adapter contract — doesn't
   implement it. Prerequisite for any effects-adjacent phase.

## 5. Recommendation

**Defer M4 unification.** The categorical story (ADR-0158/Sanada; MTT
modalities) exists and the row-as-mode representation is feasible, but
every checking payoff requires either ambient mode contexts — out of
scope, research-grade — or re-implementing coverage/residual/row-poly
machinery behind a mode-algebra interface strictly poorer than the
codomain-former machinery it replaces. `IO<sig>` remains canonical;
modes keep the same honest-bound posture (`unresolvable → pass`) that
symbolic `IO(s)` sigs already enjoy.

**Optional cheap subset (gated on M2 landing)**: register the effect
row as a canonical mode-algebra instance in the P2 registry —
`mode.algebra.effects` = `{ compose: row-union-with-entry-merge,
unit: {}, order: row-width subsumption }` — so users *may* annotate
`A ~[{io: S}]~> B` as a declared effect row. Optionally
`synth_graded_arrow` cross-checks an `io`-headed mode against an
`IO`-headed codomain when both are present (one row comparison).
Buys: a second real algebra exercising the registry (de-risks M2's
junk-drawer risk) and forward-compat for later unification, at zero
risk to `IO<sig>` semantics. Do **not** attempt: moving
coverage/residual/row-poly checking into modes, ambient mode
contexts, or `perform` minting modes.

If the subset is also deferred, M4 closes as: *assessed — deferred;
revisit only if full-MTT mode contexts land, or a second consumer
(e.g. session types composing with effects, ADR-0203) needs effect
rows as modes.*

**Prerequisite regardless of M4**: port `synth_handle` /
`synth_perform` / `check_call`'s perform branch into
`typecheck_model.hnl` so the canonical model covers what NBE already
checks.
