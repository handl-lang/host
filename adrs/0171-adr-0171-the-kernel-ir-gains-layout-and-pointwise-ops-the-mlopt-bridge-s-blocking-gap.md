+++
id = "ADR-0171"
title = "ADR-0171: The kernel IR gains layout and pointwise ops — the mlopt bridge's blocking gap"
status = "open"
priority = "med"
+++

---
title: The kernel IR gains layout and pointwise ops
type: adr
date: 2026-11-19
status: accepted
status_note: "Accepted — ops implemented and tested; user sign-off 2026-11-20 (plan 0039 go)"
applies_to: '`lib-kernel-ir` (ops), `kernelopt` (ir_bridge), `compile-triton` (lowering, later)'
---

# ADR-0171: The kernel IR gains layout and pointwise ops

## Decision

1. **Eleven ops join the set** (ten → twenty-one): the layout ops
   `transpose(tensor, dim0, dim1)` and `permute(tensor, dims...)`,
   and the pointwise ops `mul`, `sub`, `neg`, `relu`, `exp`,
   `sigmoid`, `tanh`, `gelu`, `silu`. Each declares signature, grade,
   cost and torch denotation like every existing op.
2. `transpose`/`permute` are **views**: they denote torch relayouts
   that move no bytes, so they are priced at zero (the `pipeline`
   precedent — cost is a lower bound; a backend that materialises the
   relayout pays at lowering, which the referee measures).
3. Pointwise costs are honest elementwise prices: binary ops read
   both operands and write the broadcast output at 2 B/elem with one
   `ops` count per output element; unary ops read+write likewise.
   Grades follow rounding: `mul`/`sub`/transcendentals are `ROUNDS`;
   `neg`/`relu` are `EXACT` (selection/sign-flip, no rounding).
4. This extension exists **because the bridge measured the gap**:
   without a transpose, `matmul(W[m,k], v[k])` and `linear` — the two
   most common decoder spellings — cannot be expressed at all.
   Vocabulary grows by demonstrated need, not speculation; the next
   extension still requires its own ADR.
5. `permute` takes its dims as **positional int args** (the wire
   format carries ints, not lists); `transpose` likewise. Attr-carrying
   mlopt terms are translated by the bridge reading `dim0`/`dim1`/
   `dim`/`dims`/`order` attrs — the kernel-ir ops themselves stay
   positional-only.

## Why

The Phase-C bridge spike found the ~95-op mlopt IR overlaps the
kernel IR on **two ops**. The blockers were exactly layout
(`transpose`/`permute` — no way to express `[out,in]` weight layouts
against `dot`'s contracted-dim-first `[k,n]`) and the pointwise tail
(`mul`/`sub`/`neg`/activations — no counterpart for bias-add
followers). Every added op has an unambiguous torch denotation, so
the extension costs no new semantic machinery — `Term.denote`,
`to_json`/`from_json`, and the cost walk already handle them.

## Not doing

- **Reductions** (`sum`, `mean`, `max`) — no measured bridge demand
  yet; `reduce` is the split-k partial combiner, not a tensor
  reduction, and conflating them would be a name collision like
  mlopt-`split` vs kernel-`split`.
- **Norms/attention** (`rms_norm`, `layer_norm`, `softmax`, `sdpa`)
  — fused nonlinear kernels, not pointwise; each needs its own
  measured case before admission.
- **Casts/dtype ops** (`to`, `float`, `bfloat16`) — the dtype gap is
  already documented as a semantic gap of the bridge; a `cast` op
  without a dtype story would fabricate meaning.
- **`reshape`/`flatten`/`expand`** — view ops with shape arithmetic
  the positional-arg wire format handles awkwardly; revisit if the
  bridge meets a real case.
- **Repricing `bias`** — its under-priced large-add cost is a known,
  recorded asymmetry (cost is a bound, not a ranker); fixing it is a
  separate change.

## Enforcement

- `lib-kernel-ir/src/lib_kernel_ir/ops.py` — `OPS` is 21; the new
  entries carry signature/grade/cost/denotation.
- `lib-kernel-ir/tests/test_ops.py` — vocabulary pin updated; new
  denotation tests for each op.
- `kernelopt/src/mlopt_kernel/ir_bridge.py` — `matmul` weight-first,
  `linear`, `transpose`, `permute`, and the pointwise set translate;
  everything else still declines by name.
- `kernelopt/tests/test_ir_bridge.py` — positive tests prove the
  previously-declined spellings denote correctly under torch.
