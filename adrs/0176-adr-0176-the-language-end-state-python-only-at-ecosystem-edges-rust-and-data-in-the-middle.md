+++
id = "ADR-0176"
title = "ADR-0176: The language end-state — Python only at ecosystem edges, Rust and data in the middle"
status = "done"
priority = "med"
+++

---
title: The language end-state — Python only at ecosystem edges
type: adr
date: 2026-11-21
status: accepted
status_note: "Accepted 2026-11-21 (user: 'rust end-state approved'). Feasibility spike landed: host/project/reports/rust-adapter-spike.md — safetensors/hf-hub/onnx-ir all work pure-Rust (whisper-tiny verified); highest-leverage port is the ONNX→IR lifter (~90-op table); candle executes but does not lift; whisper-decoder blocked on mutation ops; kokoro stays Python. Refines scope, not direction."
applies_to: 'adapter/binding layer — mlopt adapters, kit-torch/kit-onnx (future), meta-tier drivers, host-tensor consumer side'
---

# ADR-0176: The language end-state

## Decision (accepted)

Handwritten Python survives **only at ecosystem edges** — backend
bindings that cannot exist without Python's ecosystem. Everything
with a schema becomes data or Rust:

```
Rust:    kernel · engines · compile-* · eqsat/engine-eqsat · host
Data:    fragments · architectures-as-specs · laws · costs ·
         pipelines (.hnl) · minted records
Python:  HF-architecture front-end (new model families only) +
         optional meta-tier drivers
.hnl:    libraries, scripts, artifacts — the user-facing layer
```

### The three moves

1. **Rust adapters for declarative formats.** ONNX (protobuf —
   `onnx-ir`/prost, `ort` for runtime) and safetensors (HF's own
   Rust crate; `hf-hub` for downloads) need no Python, ever.
2. **Architectures-as-data.** A model family becomes a fragment
   spec — a declared composition of `tensor.*` ops — plus
   safetensors weights bound as leaves. Whisper encoder ≈ 15 op
   patterns. Once specced, optimizing that family needs **zero
   Python installed** — a differentiated claim vs every
   torch-based optimizer.
3. **sdkgen for residual FFI.** Where a backend exposes only a
   native/FFI surface, a schema→bindings generator emits Rust
   bindings (the `aten_port_gen` pattern targeting Rust; prior art:
   tch-rs, candle). Generated, drift-guarded — never handwritten.

### What stays Python (honest edge)

- **HF architecture discovery** for *new* model families —
  transformers architectures are Python code; until someone writes
  the fragment spec, a thin Python front-end lifts them. One-time
  per family, not per run.
- **Meta-tier drivers** (gamekit/discoverykit/benchkit
  orchestration) — research-loop code where velocity >
  performance; `.hnl run` pipelines already absorb orchestration,
  and hot paths go Rust via engine-eqsat regardless.

## Why

- Every surviving Python line is a maintenance + correctness cost;
  the audit trail shows handwritten tables are where bugs hide
  (norm arity, codec drift). Data and generated code can't drift
  silently.
- The product claim "optimize ONNX/safetensors with no Python"
  is only possible if the middle is Rust — Python at the edges is
  a feature (ecosystem reach), not a foundation.
- Consistent with ADR-0175: adapters were always kit data +
  bindings; this ADR says the bindings themselves minimize.

## Not doing

- **No Python elimination theater.** HF-front-end and meta
  drivers stay Python where Python is genuinely the ecosystem.
  Forcing Rust there costs more than it saves.
- **No candle/ort commitment yet** — the spike evaluates whether
  candle (HF's Rust ML framework) already loads safetensors+archs;
  if it does, the adapter scope shrinks dramatically. Decision
  follows evidence.
- **No architecture-spec format freeze** — the fragment spec
  schema lands with P1 consumers; this ADR commits the direction,
  not the syntax.

## Consequences

- New adapter work defaults to Rust/data; Python adapters are
  legacy-edge, documented as such.
- `kit-torch`/`kit-onnx` (P3 names) inherit this boundary: thin
  bindings in, everything else data.
- The "no Python installed" claim becomes a testable milestone:
  ONNX in → optimized artifact out, Rust-only.
