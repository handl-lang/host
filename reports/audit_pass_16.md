---
title: Audit pass 16 — mlopt streaming/F32 sidecar verified clean, qwen2+distilbert arch families audited (one spec dtype bug), gpt2 canonical PASS sound, D-1d repo exists but is empty
type: report
date: 2026-11-16
---

# Audit pass 16

Scope: committed state since pass 15 — mlopt `a569845` (streaming
safetensors sidecar writes + dtype-native entries), source-onnx
`ad48a19` (qwen2 + distilbert arch families, now committed — the
pass-15 WIP landed), the gpt2-small canonical verify-run report
(`gpt2_e2e_canonical.md`), the D-1d `kit-discovery` split state, and
cross-repo pin consistency after the tag wave (gamekit `v0.1.11`,
discoverykit `v0.1.17`).

**WIP present and not disturbed:** graphopt has ~400 uncommitted
D-1d cascade files (226 `D` + 174 `M` — `packages/mlopt-discovery`
deleted, pyproject repinned); mlopt has the matching 9-file rename
cascade plus four untracked run artifacts (`arch-gpt2-small.*`,
`arch-t5-small.*`); kernelopt has 18 modified files including the
same pyproject repin; kit-discovery itself has 236 uncommitted
files (the `mlopt_discovery → kit_discovery` rename, staged `A`/`R`
entries, HEAD still `1d6429f`).

## Per-commit verdicts

### mlopt `a569845` — CLEAN; all three audit questions verified

**(a) `write_safetensors` streaming is byte-correct**
(`src/mlopt/emit.py:2542-2595`). `data_offsets` are computed up
front as `prod(shape) × _SAFETENSORS_ITEMSIZE[dtype]` — correct for
both dtypes (8/4, `emit.py:2539`). Both loops iterate
`tensors.items()` in the same insertion order, so header offsets
and payload writes agree; the header is `sort_keys=True` JSON but
lookup is by name, so ordering is irrelevant to `tensor.load`.
The per-tensor `len(blob) != want` check (`emit.py:2587-2593`)
refuses shape/flat mismatches that the old accumulate-in-memory
version would have written silently corrupt — strictly stronger.
Scalar/empty shapes degenerate correctly (`nbytes = itemsize` for
`shape=()`, `0` for a zero dim → empty blob passes the check).
Non-contiguous/strided flats are normalized by
`np.ascontiguousarray` in `_sidecar_payload` (`emit.py:2526`).

**(b) F32 zero-copy does not break f64-expecting consumers.**
Traced every reader:

- `_as_array(f, "F32")` (`emit.py:2466-2488`): f32 torch →
  `detach().cpu().numpy()` → `np.asarray(…, float32)` →
  `reshape(-1)` is a genuine view chain — no f64 materialization,
  claim accurate. f64 inputs still widen-then-round through
  `np.asarray(…, float32)` (deliberate, documented).
- Host side: `tensor.load` accepts `"F32"` and widens `f32 as f64`
  (`host-tensor/src/ops/codec.rs:632`) — exact.
- Python side: `_load_safetensors` maps `"F32" → "<f4"`
  (`adapters/onnx_rust.py:211-221`), `_param_tensor` keeps f32 as
  `torch.float32` (`onnx_rust.py:286` — only f16/bf16 upcast).
- Verify-run: the f64 reference attempt upcasts param values
  `.to(float64)` (`cli.py:561-563`) and `_f64_args` upcasts float
  inputs (`cli.py:274-284`); f32→f64 is exact on both sides, so the
  compare sees identical values. The gpt2 run's
  `max_abs=1.79e-12` is consistent with this.
- `emit_program`'s sidecar write happens before return while `feed`
  (holding the torch tensors the views point into) is still alive;
  `_feed_expr`'s inline-literal path still calls `_as_array(f)` at
  the F64 default (`emit.py:2455`) — unchanged.

**(c) `del param_leaves_np` is safe** (`adapters/arch_rust.py:468`).
`_bind_params` converts every leaf through `_param_tensor` →
`torch.tensor(np.ascontiguousarray(arr))` (`onnx_rust.py:286`),
which always copies — `param_values` holds torch tensors with no
reference to the memmap views. `_apply_transform` results are
consumed within the loop iteration (`arch_rust.py:372-385`); the
alias check (`:390-396`) reads only dict keys. After `del`, CPython
refcounting drops the last array refs → the `np.memmap` `buf`
inside `_load_safetensors` unmaps. No dangling-reference path.

**Tests:** `pytest tests/test_emit.py -k "safetensors or sidecar"`
→ 7 passed, including the contiguous-offsets layout assert and the
F32 rounding test. No test pins the new mid-write refusal or the
zero-copy path specifically — INFO only.

### source-onnx `ad48a19` — emitters honest; one spec data bug (LOW)

**qwen2** reuses the llama path via a `CausalLm` struct
(`src/arch.rs` ~483-505, `causal_forward`/`causal_layer`/
`causal_attention`): GQA `repeat_kv` before scores, RoPE on
unexpanded k, `head_dim^-0.5` post-matmul scale, pre-RMSNorm +
residual, SwiGLU bias-free MLP — all matching HF Qwen2.  Verified
against primary sources:

- `attention_bias: true` is **correct**: HF hard-codes q/k/v
  `bias=True`, o_proj/MLP `bias=False` for the qwen2 architecture
  (transformers PR #32893 discussion; the cached AWQ sibling's
  safetensors empirically carries all 72 `*.self_attn.{q,k,v}_proj.
  bias` tensors — I parsed the header). `o_proj`/`mlp` bias-free
  matches.
- Dims match both the AWQ sibling's config.json (read locally:
  hidden 896 / 24 layers / 14q·2kv / intermediate 4864 / vocab
  151936 / rope_theta 1e6 / rms_norm_eps 1e-6) and the dense
  checkpoint's config.json (fetched from HF — identical dims).
- `tie_word_embeddings` alias `lm_head.weight →
  model.embed_tokens.weight` with `TransposeLastTwo` is the
  correct F.linear spelling.
- The `rms_norm` eps fix (`arch.rs:4117-4124`) is a genuine
  correctness repair: it now reads `section.layer_norm_eps`, and
  `llama_emitter` sets that field from `l.rms_norm_eps`
  (`arch.rs:2903`) — llama behavior unchanged, qwen2 no longer
  inherits llama's default.

**distilbert** is a faithful post-LN encoder slice: word+position
embedding sum (no token-types — correct for DistilBERT),
`embeddings.LayerNorm`, per-layer `sa_layer_norm(x+attn)` then
`output_layer_norm(x'+ffn(x'))` — matching HF
`DistilBertLayer`/`TransformerBlock` order; all linears biased
(`BiasSpec::default()` = all-true, `arch.rs:329-355`);
`q@k^T * head_dim^-0.5` → softmax → `@v` → `out_lin`; position
slice `0..seq` matches HF's `position_ids = arange(seq)`; seq cap
`max_position_embeddings` validated (`arch.rs:1596-1602`);
MLM/pooler heads honestly out of slice and documented.

**Fabricated-bind tests are honest test design.** `fabricated_store`
(`tests/arch.rs`, new helper) mints safetensors from the assembly's
own manifest — vocab shrunk 151936→4096 / 30522→4096 and qwen2
layers 24→4 *inside the spec JSON before assembly*, so spec, term,
and fabricated checkpoint stay mutually consistent; the commit
message and test comments say "fabricated" plainly, and the
real-checkpoint tests skip-by-name when the checkpoint is absent
(both verified skipping/passing locally — `cargo test --test arch
qwen2` 7/7, `distilbert` 4/4). The AWQ-sibling refusal test runs
against the *real* cached int4 checkpoint and passes. **Caveat
(INFO):** `fabricated_store` derives each tensor's dtype from
`leaf.dtype` — i.e., from the spec itself — so fabricated binds are
self-consistent by construction and cannot catch spec↔checkpoint
factual drift. That is exactly the hole the finding below fell
through.

**Finding:** `data/architectures/qwen2-0.5b-instruct.json` declares
`"weight_dtype": "float16"` — inherited from the AWQ sibling's
`config.json` (`torch_dtype: float16`, verified locally) — but the
spec's `checkpoint.repo` names the **dense**
`Qwen/Qwen2.5-0.5B-Instruct`, whose config.json says
`"torch_dtype": "bfloat16"` (fetched from HF). `verify_weights`/
`bind_weights` hard-refuse on `dtype_name != leaf.dtype`
(`arch.rs:2058-2063`, `2122-2127`; `dtype_name` maps `BF16 →
"bfloat16"`, `arch.rs:2466`), so the env-gated real bind will fail
by name — fail-loud, never silent — but the shipped spec cannot
bind the checkpoint it names. One-word fix: `"float16" →
"bfloat16"` (the downstream f32-upcast path in mlopt already
handles bf16 payloads).

### gpt2 canonical verify-run (`gpt2_e2e_canonical.md`) — methodology sound

- Command is the real product path: `mlopt export 'arch:gpt2-small'
  --sidecar --sidecar-dtype F32 --verify-run`.
- Numbers check out: 3,216,448 compared values = 50,257 vocab × 64
  positions (gpt2-small `[1,64,50257]` logits); `max_abs=1.794e-12`
  vs atol/rtol 1e-9 is plausible f64-vs-f64 noise, and the F32
  sidecar is lossless here because gpt2 checkpoint params are
  already f32 (f32→f32 write, f32→f64 exact on both sides of the
  compare — see the a569845 trace above).
- Honesty markers present: synthetic probe inputs disclosed,
  `MLOPT_EQSAT_TIMEOUT`/`MLOPT_HNL_RUN_TIMEOUT` overrides named,
  the `uv sync` failure + `uv pip install --no-deps` workaround
  documented (see D-1d below — still unfixed), uncommitted
  source-onnx WIP disclosed, artifacts left untracked deliberately.
- One inherent property worth noting (not a defect): `--verify-run`
  re-emits through `emit_program(…, sidecar=sidecar,
  sidecar_dtype=…)` (`cli.py:451-456`), which **rewrites the shipped
  sidecar file** — deterministic same-dtype rewrite, harmless here,
  but it means the gate mutates the artifact directory.

### D-1d kit-discovery — repo now exists but is EMPTY; ordering risk is live

- `git ls-remote origin` (ssh **and** https) returns **empty, rc=0**
  — `handl-lang/kit-discovery` exists on GitHub with zero refs.  No
  tags (`git tag -l` empty), HEAD still `1d6429f`, 236 files
  uncommitted.
- **`uv sync` in mlopt fails today** — verified:
  `uv sync --dry-run` → `failed to fetch tag 'v0.1.0' … couldn't
  find remote ref refs/tags/v0.1.0`.  The `.venv` only works via
  the report's `uv pip install --no-deps <kit-discovery checkout>`
  workaround — a **non-editable snapshot** of the worktree, so it
  will silently drift as kit-discovery's uncommitted rename
  evolves.
- `fail_under` placeholder is **resolved**: `pyproject.toml:118`
  now reads `fail_under = 82` (was `0` in pass 15) — a real-looking
  measured floor; spot-verify with a coverage run before the split
  ships (INFO).
- Three consumer cascades all pin `kit-discovery @ tag v0.1.0`:
  graphopt worktree `pyproject.toml:147`, mlopt worktree
  `pyproject.toml` (`[tool.uv.sources]`), kernelopt worktree
  `pyproject.toml:57`.  None committed.  P15-N1's ordering warning
  stands verbatim and is now *demonstrated*, not hypothetical.

### Cross-repo pin consistency — consistent except the known drifts

| consumer pin | tag exists? | notes |
|---|---|---|
| eqsat `v0.1.9` (all consumers) | yes | HEAD `4e2dc87` — drift persists (law regens + D-1d doc rename); still no `v0.1.10` (P15-N2 unchanged) |
| graphopt `v0.1.16` | yes = HEAD `935b061` | consistent |
| kit-carriers `v0.1.1` | yes = `b4545cf` | consistent |
| kit-cuda `v0.1.0` | yes = `5be7310` | consistent |
| kit-onnx `v0.1.1` | yes | consistent |
| gamekit `v0.1.11` / discoverykit `v0.1.17` | yes | worktree pins current; committed mlopt/kernelopt still on `v0.1.10`/`v0.1.14` — valid tags, just behind (cascade will bump) |
| kit-discovery `v0.1.0` | **no — repo empty** | the blocker above |

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P16-N1 | MEDIUM | kit-discovery repo + graphopt/mlopt/kernelopt worktree `pyproject.toml` | D-1d is still entirely unlanded: the GitHub repo exists but has **zero refs** (`ls-remote` empty over ssh and https), no `v0.1.0` tag, 236 uncommitted rename files, HEAD `1d6429f`. All three consumer worktrees already pin `kit-discovery@v0.1.0`; `mlopt uv sync` **verified failing** (`couldn't find remote ref refs/tags/v0.1.0`) with only a non-editable `uv pip install` snapshot keeping the venv alive — silent-drift risk. Land order unchanged: commit → push → tag → real floor, *then* the consumer cascades. |
| P16-N2 | LOW | `source-onnx/data/architectures/qwen2-0.5b-instruct.json` (`"weight_dtype": "float16"`) | Spec names the dense `Qwen/Qwen2.5-0.5B-Instruct` checkpoint (bf16 per its config.json) but declares f16 — inherited from the AWQ sibling's config. `verify_weights`/`bind_weights` will refuse `dtype bfloat16, spec wants float16` by name once the checkpoint resolves: fail-loud, but the committed spec cannot bind its own checkpoint. Fix: `"bfloat16"`. |
| P16-N3 | INFO | `source-onnx/tests/arch.rs` `fabricated_store` | Fabricated binds derive tensor dtype/shape from the spec's own manifest — self-consistent by construction, so they exercise machinery but cannot catch spec↔checkpoint factual drift (the P16-N2 hole). Real-checkpoint tests skip honestly when absent; the AWQ refusal test does run against the real cached checkpoint. Acceptable design; just don't read a green fabricated bind as checkpoint-verified. |
| P16-N4 | INFO | `mlopt/src/mlopt/emit.py:2583-2594` | If the per-tensor length check fires mid-write, a truncated safetensors with a valid-looking header remains at `path` (emit still declines loudly; next run overwrites via `"wb"`). Cosmetic. |
| P16-N5 | INFO | `mlopt/src/mlopt/cli.py:451-456` + `--sidecar-dtype F32` | verify-run re-emits and rewrites the shipped sidecar in place, and an F32 sidecar over genuinely-f64 params would compare host-rounded vs torch-unrounded → spurious FAIL at the tight f64 bound. Fail-loud direction only; no silent-pass path. |
| P16-N6 | INFO | eqsat `v0.1.9` vs HEAD `4e2dc87` | Tag drift persists (three law-corpus regens + D-1c/D-1d doc renames unreachable to pinned consumers). Still worth a `v0.1.10`; no functional gap. |
| P16-N7 | INFO | kit-discovery `pyproject.toml:118` | `fail_under = 82` replaces the `0` placeholder — plausible measured floor; confirm with one coverage run before the split ships. |

No HIGH findings.  `a569845` survives the closest reading: the
streaming writer is offset-exact with a real corruption refusal,
the F32 path is value-identical across host (`codec.rs:632`),
Python (`onnx_rust.py:211-289`), and the verify-run f64 upcast —
and `del param_leaves_np` has no retention path.  The arch
emitters are honest ports (qwen2 bias/eps/alias semantics
verified against HF sources and the real AWQ safetensors header;
distilbert post-LN order matches `TransformerBlock`), with one
committed spec carrying a wrong `weight_dtype` that fails loudly
rather than silently.  The gpt2 canonical PASS is methodologically
sound.  The one action item is unchanged from pass 15 but now
demonstrated: kit-discovery must be committed, pushed, and tagged
`v0.1.0` before any consumer cascade lands, or `uv sync` stays
broken workspace-wide.

**Verdict: PASS with one MEDIUM** — all committed code since pass
15 audits clean; the MEDIUM is the still-unlanded D-1d split, now
with the GitHub repo confirmed empty and `uv sync` confirmed
broken in mlopt.
