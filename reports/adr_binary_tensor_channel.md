+++
id = "ADR-0179"
title = "ADR-0179: Binary tensor payloads on the `__tensor__` wire — dtype-preserving tensor storage"
status = "implemented"
status_note = "Implemented 2026-11-16 — kernel 9d9e875, host-tensor 3af18c0+c488bad, graphopt 069d7db+eadddba, tagged graphopt v0.1.18; verified llama1 6.1e-14, llama4 PASS 2.05GB, llama8 PASS 3.0GB (was OOM), llama22 pending retry"
priority = "high"
applies_to = '`kernel` (handl-value), `host-tensor`, `graphopt` (mlopt-kit hnl-run, mlopt-host-tensor wire.py), `mlopt`'
---

---
title: Binary tensor payloads on the `__tensor__` wire — dtype-preserving tensor storage
type: adr
date: 2026-11-17
status: implemented
status_note: Implemented 2026-11-16 — kernel 9d9e875 (handl-value TensorData::{F64,F32} + __tensor_bin__ codec), host-tensor 3af18c0+c488bad (codec bin arm + ranged safetensors payload-as-wire tensor.load), graphopt 069d7db+eadddba (hnl-run materialize egress + wire.py decode), tagged graphopt v0.1.18. Verified: llama1 6.1e-14, llama4 PASS 2.05GB, llama8 PASS 3.0GB (was OOM), llama22 pending retry.
---

# ADR-0179: Binary tensor payloads on the `__tensor__` wire — dtype-preserving tensor storage

## Context

`mlopt export --verify-run` is memory-walled at llama-22L scale
(`host:project/reports/llama_e2e_retry.md`). Two costs stack on every
tensor crossing the `Function` boundary:

1. **Transient:** `tensor.load` returns f64 data through
   `Function::call`'s `serde_json::Value` — ~32 B/element of JSON DOM
   plus an f64 `Vec` plus a `TaggedValue::Tensor` copy ≈ 40 B/elem.
   The 65.5M-element `embed_tokens`/`lm_head` spike is ~2.6 GB
   transient per load; every op call re-serializes every tensor arg
   (`tagged_to_value` at each `call_host`/`ArrSource::Foreign` site).
2. **Live:** do-block bindings stay `TaggedValue::Tensor` =
   `Rc<Vec<f64>>` — ~8 B/elem. All 22L weights ≈ 8.8 GB > the ~8.5 GB
   host budget. host-tensor's ranged safetensors reads (c488bad)
   removed the I/O wall; the wire contract is what remains.

The wire form is `{"__tensor__": {"shape": [...], "data": [...],
"dtype": "f64"}}` — `dtype` exists but only `"f64"` is legal
(`host-tensor/src/ops/codec.rs:88`, `handl-value/src/lib.rs:936`).
`tensor.load` already widens F32 sidecar payloads `f32→f64` on ingest
(`codec.rs:774`) — exact, since every f32 is an f64 — which means the
*value* of an F32 tensor is fully determined by its f32 payload.

## Decision

Extend the `__tensor__` wire record with a binary `data` spelling and
make tensor storage dtype-preserving (F64/F32 only — no promotion
lattice). Concretely:

**Wire (additive, threshold-gated).** `"data"` may be the existing
array, or `{"__tensor_bin__": "<base64 little-endian payload>"}`.
The binary spelling requires `"dtype": "f32"|"f64"` (lowercase) and
`payload.len() == numel × itemsize`. Encoders emit the binary form
iff `numel >= TENSOR_BIN_THRESHOLD` (proposed `4096`, matching
mlopt's `_SIDECAR_MIN_ELEMS` sidecar gate); below it the wire is
byte-identical to today, so all existing payloads, tests, and
debugging flows are unchanged. Non-finite elements ride the binary
channel natively (raw IEEE bits) — strictly better than the
`__nonfinite__` tags, which remain the array form's spelling.

**Storage.** `handl-value`'s `TensorValue.data` becomes
`Rc<TensorData>` where `TensorData::{F64(Vec<f64>), F32(Vec<f32>)}`
— the "kernel-first" enum the host-tensor README already anticipates
for dtype promotion, scoped to the two dtypes the sidecar produces.
`PartialEq`/`Hash` compare the **f64 widening** (f32→f64 is exact
and injective), preserving today's structural-equality semantics;
`tensor()`/`TensorValue::new` keep their `Vec<f64>` signatures
(wrapping `F64`). `host-tensor`'s `Tensor` is **unchanged**
(`Vec<f64>`): `decode_tensor` materializes the widening per call, so
all ~200 ops are untouched and compute stays f64 — bit-identical to
today's ingest-then-compute pipeline.

**`tensor.load`.** The large path skips decoding entirely: the
safetensors payload range *is* little-endian F32/F64, so `Load`
returns `{"__tensor__": {"shape", "dtype", "data":
{"__tensor_bin__": b64(payload)}}}` straight from the ranged read.
Small tensors keep the existing decode→`encode_tensor` path. No new
op, no signature change, no `.hnl` emit change — mlopt's existing
`tensor.load("<file>", "<leaf>")` spelling and `--sidecar-dtype F32`
are exactly what unlocks this.

**Egress.** `handl_value::materialize_tensor_bin(&mut Value)`
expands `__tensor_bin__` records back to the array form (widened,
`dtype: "f64"`, `__nonfinite__` tags) for process-boundary printing;
`hnl-run` calls it before `println!`, preserving the stdout contract
`KitRunner`/`verify-run` decode (`wire["data"]` is a list). The form
is self-contained — `check_portable` passes it unchanged, and it
survives arrow-`binds` serialization, module export, and
cross-process decode for free (unlike any handle/arena design).

**Honest declines.** Unsupported safetensors dtypes (F16/BF16/I64/…)
remain named `EvalError`s. A malformed bin record (bad base64, wrong
length, `dtype` absent/`"f32"`+array) is a `TypeMismatch` in
host-tensor and the established lenient object-fallback in
handl-value. `hnl`/`hnl run` and other `Value` egress points must
call `materialize_tensor_bin` before printing — tracked as a
follow-up, not silently left emitting base64.

## Alternatives considered

- **(b) Lazy handle** (`tensor.load` returns a path/name ref resolved
  at use-site): rejected. Tensors are values in the calculus —
  structural equality, hashing, `tensor.to_list`/`shape` all need the
  payload; laziness would force a `Tensor` repr change in *both*
  codecs plus resolution inside every op, and `Function::call`
  returns `serde_json::Value` regardless, so the handle cannot cross
  the boundary without one. Strictly more invasive, weaker semantics.
- **(a′) Process-local tensor arena / `__tensor_ref__` ids:** zero-copy
  per crossing but introduces global state, dangling-ref risk across
  processes (`check_portable` must reject it — the `__resource__`/
  `__compiled__` precedent), unbounded growth under `hnl-run
  --repeat`, and a second resolution path in both codecs. Deferred as
  a Phase-2 optimization *if* profiling shows base64 memcpy is the
  wall — it is not: f64 CPU compute dominates (~230 s for 1L; codec
  overhead is noise at 22L).
- **(c) mmap/zero-copy F32 sidecar:** the safetensors payload is
  already contiguous LE f32 — an mmap-backed `TensorData::Mapped`
  variant is the true zero-copy end-state. Deferred: unsafe/lifetime
  surface, page-cache RSS semantics, and it does not address the
  per-call `Value` crossing (the bigger transient). Natural Phase-2
  companion to arena refs.
- **(d) Spill-to-disk tensor arena:** same global-state objections as
  (a′) plus I/O on the hot path; rejected.
- **Full dtype lattice** (host-tensor README's "kernel-first" path:
  `Tensor.data` enum + per-op promotion rules): rejected for this
  ADR — promotion lattices are where the bugs live; F64-compute over
  F32-stored weights needs no lattice because widening is exact.

## Consequences

- **Memory:** live weights drop 8→4 B/elem under F32 sidecars
  (22L llama ≈ 8.8 GB → ≈ 4.4 GB); per-call transient drops ~6–10×
  (65M-elem tensor: ~2.6 GB JSON DOM → ~350 MB base64 string →
  262 MB `Vec<f32>`). `--verify-run` at 22L becomes feasible on an
  ~8.5 GB host *with* `--sidecar-dtype F32` (F64 sidecars still fix
  the transient but not the live set — honest note in docs).
- **Bit-identical numerics:** F32 ingest widens exactly as today;
  every op still computes f64. The 8d5f8bb F32-sidecar tolerance
  widening is **unchanged and still required** — weights are
  f32-quantized at sidecar write; the host computes from quantized
  weights either way. If anything the story simplifies: "the host
  tensor is f64-valued with f32 storage" is one sentence, not a
  pipeline accident.
- **Blast radius:** `kernel` (handl-value: `TensorData`, `TensorValue`
  eq/hash/accessors, both codec arms, threshold const, materialize
  helper, `base64` dep); `host-tensor` (`codec.rs` decode/encode bin
  arms + `Load` payload path, promote `handl-value` to
  `[dependencies]`, tests, README/AGENTS wire docs); `graphopt`
  (`mlopt-kit` hnl-run materialize-before-print; `mlopt-host-tensor`
  `wire.py` `__tensor_bin__` decode arm); `mlopt` — **no code change**
  (emit/`tensor.load` spelling and tolerance logic untouched).
- **Perf:** per weight-op call adds one base64 decode + f32→f64 widen
  (~50–500 MB memcpy-class work). At ~500 tensor.load calls + op
  crossings for 22L this is tens of seconds against an ~hours-long
  f64 verify — acceptable; revisit only with measurements.
- **Follow-ups (not this ADR):** `hnl run`/other `Value` egress
  materialization; arena-ref or mmap `TensorData` variants if
  profiling demands; dtype-preserving `host-tensor Tensor` (the full
  lattice) only if f64 materialization itself becomes the wall.
  NOTE: the 1L verify-run 4.5e-7 divergence flagged in
  `llama_e2e_retry.md` is already root-caused (graphopt `3e724a8`
  `dtype_ctx` — reference-side creator ops minted at ambient f32;
  fixed, measured 6.1e-14 on the 1L case).

## Open questions

- `TENSOR_BIN_THRESHOLD` value and whether it wants an env knob
  (`HANDL_TENSOR_BIN_*`) for tuning without rebuilds.
- Should `tensor.load` emit bin *unconditionally* ≥threshold
  (proposed — transparent) or behind an opt-in third arg?
- base64 crate dep vs a vendored ~60-line codec (house style leans
  vendored for tiny formats; base64 is ubiquitous — decide at impl).
- Does `check_portable` need a doc note that bin payloads are
  portable-but-large (a 4 GB tensor serializes ~5.3 GB of base64)?
