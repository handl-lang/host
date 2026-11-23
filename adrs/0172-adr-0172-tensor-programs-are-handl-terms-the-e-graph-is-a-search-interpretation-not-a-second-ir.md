+++
id = "ADR-0172"
title = "ADR-0172: Tensor programs are Handl Terms — the e-graph is a search interpretation, not a second IR"
status = "open"
priority = "high"
+++

---
title: Tensor programs are Handl Terms
type: adr
date: 2026-11-21
status: accepted
status_note: "Accepted 2026-11-21 — user ratified FULL convergence: tensor programs are Handl Terms, the e-graph is a search interpretation engine. Spike evidence (graphopt/docs/reports/term_ir_spike.md, graphopt@6687d29) proved lossless codec fidelity and a viable ~40-line e-graph adapter, and identified the three calculus-level gaps that must close first: (1) tree-only Terms cannot encode hash-consed DAG sharing, (2) Term::Op lacks a typed attrs slot, (3) serde_json rejects non-finite floats. Phase 0 = Term kernel extension (own ADR); codec convergence (.hnl canonical wire, hnl_to_term Source, host_surface.json) proceeds in parallel and is required either way."
applies_to: '`mlopt_core.ir` (eqsat), `mlopt_core.egraph`, `term_hnl`/`hnl_import` bridges (graphopt), tensor Language fragment (lang/kernel), Sanada reverse-handler interpretation (future)'
---

# ADR-0172: Tensor programs are Handl Terms

## Decision

1. **Tensor programs ARE Handl `Term`s — full convergence is the
   destination.** `Op{name, args, dim, source, lit}` expresses
   tensor graphs; tensor ops enter as a `Language` fragment
   (ops-as-data, ADR-0044). The e-graph is a *search
   interpretation* — an engine over Terms, alongside tree-walk,
   bytecode, cranelift, typecheck, and rewrite — not a second IR.
   Sanada's ICFP 2026 reverse-handler result is the theoretical
   license: the same term carries forward eval, typing, rewriting,
   search, and (later) verified differentiation.
2. **Phase 0 — the `Term` kernel extension (own ADR).** The spike
   proved the codec is lossless but found three calculus gaps that
   block tensor DAGs:
   - **DAG sharing**: Terms are trees; neural nets are hash-consed
     DAGs (shared weights/subterms are the norm). Needs a sharing
     construct (`Ref`/let-binding, or hash-consed cells) — a
     calculus change, not a bridge.
   - **Typed attrs slot**: `Term::Op` has no attrs field; the spike
     rode `lit` with a `__mlopt_attrs__` convention. Needs a real
     slot or a declared fragment-level mechanism.
   - **Non-finite literals**: serde_json rejects NaN/±inf; needs
     the `__nonfinite__` tagged encoding host-tensor already proved.
3. **`.hnl` / `Term` serialization is the canonical wire format —
   implemented now, required either way.** `hnl_surface`
   (term→`.hnl`) and `hnl_import` (`.hnl`→term) are the single
   translation layer; `hnl_to_term` makes `.hnl` a first-class
   `Source` (`mlopt optimize foo.hnl`). `host_surface.json` (237
   generated op signatures) is the single op-metadata artifact.
4. **`mlopt_core.ir` remains the search IR until Phase 0 lands.**
   ENodes, TensorType, typed attrs, e-classes, cost, certificates
   — unchanged meanwhile. Serialization never invents a fourth
   format: export emits `.hnl`, import accepts `.hnl`, and the
   numeric export gate (`--verify-run`) verifies artifacts.
5. **Then the migration**: tensor `Language` fragment → Term-native
   tensor programs → `engine-eqsat` (Rust or `mlopt-native`) →
   mlopt_core's typed view demoted or retired → op tables collapse
   to one codec → differential interpretation via reverse handlers.
   Each step lands behind the honesty gates already proven
   (verify-first, decline-parity, numeric export gate).

## Why

- **The distinction was artificial.** mlopt's `ir.py` already
  describes itself as a "categorical / string-diagram IR... morphisms
  in a symmetric monoidal category" — the same mathematics the
  arrow-calculus `Term` kernel is built on. Two spellings of one
  idea produced four divergent op tables and a permanent translation
  tax.
- **Completeness is per-calculus, and it transfers.** Sanada
  completeness covers Handl's calculus for computation and effects;
  the ICFP 2026 result extends the same framework to reverse
  differentiation. Adopting `Term` as the tensor representation
  inherits both — verified gradients of *optimized* programs become
  a handler away, which no parallel IR provides.
- **The e-graph objection dissolves.** The e-graph is a space of
  terms (union-find of classes), not a term — but engines already
  interpret terms without being terms. `engine-eqsat` is the same
  pattern: hash-cons, saturate, extract. Search algorithms are
  representation-agnostic machinery written once.
- **Honesty preserved.** Convergence removes translation seams where
  semantics could silently drift (the Phase-B overfire class of
  bugs); one representation, one codec, one place guards live.

## Not doing

- **No claim that categories give free e-graphs.** Saturation,
  extraction ordering, proof production, and cost models remain
  algorithms to build — Sanada completeness does not supply them.
- **No immediate Rust rewrite of mlopt_core.** The spike decides
  whether search moves to Rust (native `Term` access) or stays
  Python over serialized terms; performance data, not aesthetics,
  decides.
- **No handwaving of attrs.** Law guards (`AttrEq`, shape/dtype
  predicates) must land as typed `Language`-fragment declarations or
  `Value` payloads with the same honest `!Native` refusal taxonomy —
  the Phase-B contract carries over unchanged.
- **Ops stay domain-declared.** Tensor ops enter as a fragment;
  host-provided ops stay host-provided. Nothing becomes privileged.

## Consequences

- `term_hnl.py`, `hnl_import.py`, `hnl_surface.py`, `emit.py`,
  `bindings.py`, `modelproj.py` reduce to one codec direction —
  `Term` ↔ surface — deleting most of the bridge surface.
- `.hnl` export/import becomes identity at the term level; the
  numeric export gate (mlopt honesty bundle) verifies the artifact
  before it ships, unchanged.
- eqsat's e-graph gains a representation-agnostic node seam;
  `mlopt-native` becomes the candidate home for a Rust
  `engine-eqsat` if the spike justifies it.
- New cost: every consumer of `mlopt_core.ir` (laws, costs,
  carriers, minted.db records) migrates op-name/attr spellings to
  the fragment vocabulary — a real, staged migration tracked in
  `project/` plans, not a flag day.
