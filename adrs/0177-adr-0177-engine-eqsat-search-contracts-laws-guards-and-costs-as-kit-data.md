+++
id = "ADR-0177"
title = "ADR-0177: engine-eqsat search contracts — laws, guards, and costs as kit data"
status = "done"
priority = "high"
+++

---
title: engine-eqsat search contracts — laws, guards, and costs as kit data
type: adr
date: 2026-11-22
status: accepted
status_note: "Ratified by user 2026-11-22 — was proposed. Full reasoning: handl-lang/engine-eqsat docs/design/p2-search-contracts.md. Decides spike §6 questions 1–5 before mass law porting."
applies_to: 'engine-eqsat, kit-eqsat (future), eqsat mlopt_core.laws/cost migration, hnl optimize command surface'
---

# ADR-0177: engine-eqsat search contracts

## Decision (proposed)

Five contracts for the P2 search interpretation, fixing what mass law
porting builds on:

1. **Law format: JSON records** mirroring `laws.serialize`
   `LAW_FORMAT=2` (`version/kind/name/law/lhs/rhs/cond/dspec/tags/
   derivation/cert/error_bound/bound_norm/serializable/missing_hooks`),
   with patterns spelled in the `Pat` authoring view — `{"mvar"}`,
   `{"const": <json>}`, `{"op", dim, args, source, lit, attrs}` —
   never the compiled `handl_egraph::Term` bridge encoding. `.hnl`
   rejected (a 2-cell is data, not a program; one-codec invariant).
   Lives in `kit-eqsat`; `laws/*.json` in engine-eqsat is the
   reference corpus until the kit repo exists.
2. **Guards: port the `cond`/`dspec` DSL to Rust** as a
   guards-as-data interpreter over `BoundEnv`, staged — attr/scalar
   predicates + full dspec exprs first, shape-reading predicates
   after a kernel-side shape-inference module lands. Procedural
   Python `check=`/`derive=` hooks (~20–25% of guarded sites,
   concentrated in attention/layout) stay Python-side, flagged
   `missing_hooks`, never silently dropped.
3. **Costs: a serializable `CostSpec`** — named kinds
   (`count`/`size`/`op_table` in P2; `flops` after shape inference;
   roofline/fusion/executor as backend-profiled specs later).
   Interpreted engine-side over internal terms; the `FnMut` overload
   stays for procedural costs, flagged.
4. **Certificates: no `proof.py` port in P2.** `ProofEdge`/
   `Application`/`merge_log` provenance suffices; cert reconstruction
   stays Python-side until a P3 consumer demands replayable certs.
   Provenance records gain serde export when that consumer lands.
5. **Engine surface: a `SearchEngine` trait minted inside
   `hnl-engine-eqsat`** (not `hnl-engine`, not a contract crate) —
   `saturate`/`optimize`/`name` over `LawSet` + `CostSpec` data.
   `EGraphEngine` is the sole impl; extract the trait when a second
   search interpretation exists.

## Rationale

- `cond.py` is *already* a declarative JSON-safe DSL — the format
  exists; P2 needs a Rust reader, not a new language. Census: the
  guarded majority of the shipped + machine-shipped banks is
  `cond`/`dspec`-expressible; procedural hooks are the flagged
  minority.
- Compiled rule packs can't carry minted laws (generated records);
  a Python callback seam keeps a host alive in the hot loop (spike
  §3: Rust-over-Term is the only viable production path).
- Every shipped cost model is "per-op table + parametric formula over
  shapes" — data. Roofline/fusion/executor are backend algorithms,
  correctly deferred as named specs.
- Level-2 proof replay has no Rust-side consumer until P3 verified
  delivery; porting it now freezes a format before its consumer.
- `hnl-engine` holds kernel contracts free of domain vocabulary;
  a contract crate is speculative with one inhabitant. Concrete
  first, seam on evidence — the `play.register()` posture.

## Consequences

- `hnl-engine-eqsat` gains `law::from_data`/`LawSet`,
  `cost::CostSpec`, the `cond`/`dspec` interpreter, a kernel-side
  shape-inference module, and the `SearchEngine` trait.
- New error variants name every refusal: `UnserializableLaw`,
  `UnknownCostKind`, `GuardUnsupported` — declines stay named.
- Python keeps: roofline/fusion/executor costs, `proof.py`,
  procedural hooks, the `TensorAlgebra` dspec channels — all flagged,
  none silently weakened.
- Mass porting unblocks: laws/costs move as kit data; minted records
  project into the record format at admission.
