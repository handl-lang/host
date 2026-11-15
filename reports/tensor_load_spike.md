---
title: 'Spike: tensor.load — the missing out-of-band weight channel for .hnl artifacts'
type: report
date: 2026-11-16
---

# Spike: `tensor.load` — the missing out-of-band weight channel

Date: 2026-11-16 · Status: investigation spike — **verdict: fix is
upstream; STOP** (per `hnl_run_scale_spike.md` fix #2) · Follow-up to:
`project/reports/hnl_run_scale_spike.md` (the 206 MB whisper-encoder
`.hnl` — 79% of it `tensor.from_list([f64…])` weight literal text in
67 leaf bindings).

## TL;DR

- **No channel exists.** `host-tensor`'s entire op surface is 239
  registered `tensor.*` ops (`host-tensor/src/ops/mod.rs:316+`) —
  pure tensor algebra plus the JSON codec ops
  (`zeros`/`ones`/`from_list`/`to_list`/`shape`/`size`/`stride`/`t`/
  `to`/`view`). Grepping the registry for
  `load|file|byte|read|write|save|encode|decode|base64|serial|io`
  returns **zero** ops. `tensor.from_list` is the *only* way bytes
  enter a `Tensor`.
- **`from_list` is JSON-numbers-only by construction.** The internal
  `Tensor` is `{shape: Vec<usize>, data: Vec<f64>}` on the wire as
  `{"__tensor__": {"shape": [...], "data": [...], "dtype": "f64"}}`
  (`ops/mod.rs:28-34`); `ops/codec.rs`'s `FromList` accepts nested
  arrays of JSON numbers plus `{"__nonfinite__": …}` tags — nothing
  binary. `dtype` accepts only `"f64"` (`ops/mod.rs:772` — `tensor.to`
  to float32 "must fail loudly").
- **A `from_bytes`-style op is not even expressible today:** Rust
  `Function::call` args are `serde_json::Value`
  (`ops/codec.rs:16`) — the op ABI has **no bytes variant**, so a
  bytes-channel op would first need a `Value` extension. The
  path-based `tensor.load` below needs none.
- **The existing IO-ish libs don't help:** `lib-base64`
  (`base64.encode`/`decode`) decodes to a UTF-8 `String` and errors on
  non-UTF-8 (`lib-base64/src/ops.rs:61-66`), and base64-inlining f64
  weights is still ~1.37× of 162 MB ≈ 220 MB *of artifact text* —
  defeats the purpose. `lib-fs`'s `read_file` is a libpython-FFI hack
  reading text mode `'r'` (`lib-fs/src/index.hnl`) — binary-unsafe and
  fragile. Repo-wide grep for `tensor.load`/`tensor.from_bytes`/
  `tensor.from_file` finds only the spike report itself.

## Minimal op contract (proposal for host-tensor)

### `tensor.load(path)` / `tensor.load(path, name)`

Reads a **safetensors** sidecar and returns one `Tensor`.

- **`path`** — string. Resolution order: (1) relative to the
  `.hnl` artifact's directory when the runner knows it, (2) relative
  to the process cwd. (See "stdin-eval wrinkle" below — (2) is what
  `KitRunner` gets today.)
- **`name`** — optional string key into the safetensors header
  (`{"__metadata__"?, name: {dtype, shape, data_offsets}}`).
  Omitted ⇒ the file must contain exactly one tensor, else
  `EvalError::Other` naming the keys. This lets one
  `<artifact>.safetensors` hold all 67 whisper weights, or one file
  per leaf — both spellings work.
- **`dtype`** — accept `"F64"` (identity; internal repr is f64) and
  `"F32"` (widen `f32→f64` on ingest; documented precision note —
  whisper weights are f32 anyway, and emitting f32 sidecars halves
  the binary to ~80 MB). Reject `F16`/`BF16`/`I64`/`BOOL` with a named
  error for now; extending later is a match arm, not a format change.
- **Non-finite** — binary channel carries NaN/±inf natively; no
  `__nonfinite__` tag needed.
- **Errors** — missing file / bad magic / truncated payload /
  unknown dtype / missing `name` ⇒ `EvalError::Other` with the path
  and reason (same honesty bar as `FromList`'s ragged-input error).

Safetensors is the right container: the header is plain JSON
(length-prefixed, 8-byte LE), the payload is raw little-endian — no
new format to invent, torch already writes/reads it, and one header
carries every tensor's shape+dtype so `tensor.load` needs **no**
shape argument (shape comes from the file, not the call site — less
for emit to get wrong).

### Rejected alternative: `tensor.from_base64(b64, shape)`

Expressible today (String arg, no Value change) but keeps the bytes
*inside* the artifact: f64 weights → ~216 MB of base64 text, worse
than the 162 MB decimal text it replaces. Only worth adding later for
small tensors if a bytes channel never lands.

## Emit-side plan once `tensor.load` lands (not implemented — STOP)

- `emit_function`'s `param_feed` branch (`emit.py:~2042-2110`):
  `Param` leaves above a size threshold emit
  `name :: tensor.load("<sidecar>", "<leaf>")` instead of
  `tensor.reshape(tensor.from_list([…]), […])`; small params keep
  inline literals. Var leaves are runtime inputs — unchanged.
- `mlopt export` (`cli.py:~160-190`) writes the sidecar next to
  `--out` (`<slug>.safetensors`, F32) alongside the `.hnl`.
- Artifact size: ~200 KB text + ~80 MB binary for whisper-encoder —
  vs 162 MB today. (The remaining ~40 MB is the body expression; the
  hash-consed `__sN ::` bindings from `3e5f579` already addressed the
  worst of it.)

### stdin-eval wrinkle (must be resolved in the op or the emit path)

`KitRunner.run` (`emit.py:~2218`) pipes the program to `hnl-run -`
**stdin** — there is no artifact directory, so a relative `path`
resolves against the *caller's cwd*. Options, cheapest first:

1. `tensor.load` also accepts an absolute path; mlopt's `export`
   writes absolute paths into the artifact (portable only per-machine
   — document it).
2. A `tensor.load` env-var base (`$TENSOR_LOAD_DIR`) prepended to
   relative paths — keeps artifacts relocatable as a pair.
3. `hnl-run` gains `--asset-dir` (runner-side, bigger change).

Recommendation: (2) — one getenv in the op, no runner changes, and
`KitRunner`/verify-run paths keep working by exporting the var.

## Evidence index

- `host-tensor/src/ops/mod.rs:28-34` — `__tensor__` wire form,
  `dtype: "f64"` only.
- `host-tensor/src/ops/mod.rs:316-555` — the full `registry.register`
  list (239 ops; no IO).
- `host-tensor/src/ops/codec.rs:16,339-412` — `FromList`:
  `serde_json::Value` args, JSON-number leaves + `__nonfinite__`
  tags only.
- `host-tensor/src/ops/mod.rs:772` — `tensor.to` f32 "must fail
  loudly" (dtype is f64-only).
- `lib-base64/src/ops.rs:13,61-66` — decode → UTF-8 string, errors on
  non-UTF-8.
- `lib-fs/src/index.hnl` — `read_file` = text-mode libpython FFI.
- `mlopt/src/mlopt/emit.py:2042-2110,2186-2247` — feed→`from_list`
  rendering; `KitRunner` stdin eval.
- `mlopt/src/mlopt/cli.py:160-190` — `export` writes `<slug>.hnl`.
