+++
id = "ADR-0178"
title = "ADR-0178: P3 codec consolidation and mlopt_core.ir demotion — sequencing"
status = "done"
priority = "high"
+++

---
title: P3 codec consolidation and mlopt_core.ir demotion — sequencing
type: adr
date: 2026-11-22
status: accepted
status_note: "Ratified by user 2026-11-22 — was proposed. Evidence: host/project/reports/p3_codec_consolidation_spike.md (full table census + consumer map). Sequences ADR-0174 phase P3 and ADR-0175's eqsat→engine-eqsat/kit-eqsat + graphopt→mlopt dissolution rows; does not amend ADR-0177's contracts."
applies_to: 'graphopt (mlopt_discovery codec trio, mlopt.emit, vocab/), mlopt (emit._OPS, stage.py, adapters), eqsat (mlopt_core.ir/egraph/laws/cost/typing), engine-eqsat, kernelopt, mlopt-onnx, mlopt-host-tensor/mlopt-torch bindings'
---

# ADR-0178: P3 codec consolidation and `mlopt_core.ir` demotion

## Decision (proposed)

P3 runs as six work packages with hard dependency gates. The codec
consolidates *first* and *independently* of the engine swap; the IR
demotion is gated on measured engine-eqsat parity, never on schedule.

### The table end-state

- **Two canonical artifacts, distinct authority:** the tensor
  `Language` fragment (`ast.yaml` + `index.hnl` decls +
  `aten_schemas.json`/`aten_port_list.txt` in host-tensor) is the op
  *language*; `graphopt/vocab/` is the canonical *records* store
  (laws/handlers/prims/schemas/models). `host_surface.json` and
  `aten_classify.py` remain generated read-models — never canonical.
- **Direction flips:** `vocab/` stops being generated *from* Python
  owners and becomes the source; Python registries (`opmeta`,
  `ATTR_SCHEMA`, shape rules, binding arg-specs) become generated
  views via one `emit_views` pass, each edge `--check`-guarded.
- **One term codec:** `hnl_codec`'s kernel-`Term` envelope
  (`__nonfinite__`/`dag.program`, `Term::Op.attrs` channel) is the
  surviving term serialization. `mlopt_core.ir.term_to_data` demotes
  to a view codec and retires with the IR. `stage.py`'s record
  protocol gains a `schema:` pin.
- **Unguarded-table closure:** `mlopt_onnx` `opmap`/`liftmap` get a
  generator + drift check (the `torch_port_gen.py` pattern) — the
  last unguarded codec tables.

### The IR demotion path

`engine-eqsat` replaces `mlopt_core.egraph` as THE search path through
`ports.Engine` (`engine="eqsat"`), never by rewiring call sites:

- **D1 parity corpus** — differential extracted-term comparison vs
  `EGraph` on the discovery corpus + real lifts (the ADR-0177
  migration harness).
- **D2 opt-in** — Term bridge callable from Python (PyO3 preferred;
  `mlopt-native` precedent — one FFI call per search, hot loop in
  Rust) behind `engine=`; `EGraph` stays default + oracle.
- **D3 feature catch-up** — stage-2 guarded-law batches,
  `flops`/`op_table` cost specs, AUTO_TIER→`LawSet` projection, and a
  port-or-decline decision for every engine feature gap (`policy=`
  ordering, `truncation_level`, causal fold, certificates).
- **D4 measured default flip** — only on differential parity with no
  verify-gate regression; then `mlopt_core.egraph` demotes to
  spec/oracle.
- **D5 adapter Term-native output** — Sources/Sinks produce/consume
  kernel `Term`s; `mlopt_core.ir` becomes a thin view or retires;
  `hnl_codec` is the only translator during transition.
- **D6 retirements** — `mlopt_native` (superseded by engine-eqsat),
  the Python egraph proper, un-re-expressed procedural-hook laws
  (named declines).

### Work packages (ordered; deps in spike §4)

| WP | gate |
|---|---|
| WP0a codec guards (onnx maps, stage schema pin, supported⊆bindings census) | none — start now |
| WP0b codec envelope (attrs channel, `term_to_data` timeline) | kernel attrs landed — start now |
| WP1 engine-eqsat stage-2 guards + shape inference | upstream, in flight |
| WP2 law port batches + differential harness | WP1 for guarded laws; unconditional subset can start now |
| WP3 `engine="eqsat"` opt-in + bridge | WP0b + WP2 partial |
| WP4 measured default flip + egraph→oracle | WP3 + bench parity |
| WP5 table retirement + repo dissolutions | WP4 + ADR-0175 phase gates |

## Rationale

- The audit (pass 1, F5) closed the *guarded-table* half of invariant
  4; what remains is deeper: three term representations and two term
  wire formats. Sequencing codec-first keeps every later step
  single-translator.
- `ports.Engine` already isolates the swap — the demotion is an
  adapter problem (bridge + law/cost data), not a pipeline rewrite.
  That is why WP3 is small once WP1/WP2 land.
- Flipping `vocab/` to source-of-truth *before* the law port would
  strand the Python bank mid-migration; keeping Python owners as the
  oracle until D4 is the honest order.
- PyO3 over subprocess: a search is one saturate+extract call but
  latency-sensitive and progress-bearing; `mlopt-native` proved the
  optional-native-package pattern.

## Consequences

- New codec/table work cites this ADR: tables are manifest data or
  generated views; a new hand table must name its retirement WP.
- `engine=` gains `"eqsat"`; `EGraph` remains the correctness oracle
  permanently (differential harness is a kept test, not a phase).
- `mlopt_core.ir` retirement is explicitly *after* adapters go
  Term-native — the IR is a view during transition, never a second
  permanent representation (ADR-0174 non-goal census enforced).
- Feature gaps in engine-eqsat are named declines (`GuardUnsupported`,
  `UnserializableLaw`, `UnknownCostKind` precedent extended to
  `PolicyUnsupported`/`CertUnavailable` as needed) — never silent
  narrowing of the shipped law bank.
