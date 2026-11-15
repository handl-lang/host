---
title: Audit pass 6 — arch-family wave (bert/vit/gpt2/llama/whisper-combined) + tensor.load sidecar channel
type: report
date: 2026-11-15
---

# Audit pass 6

Scope: commits since pass 5 — source-onnx `981c755` (bert),
`208e713` (vit), `74b8dd6` (gpt2), `9192c06` (llama + primitives),
`8c6dd79` (whisper combined spec + name-keyed bound payload);
host-tensor `eec98bb` (`tensor.load` safetensors ingest); graphopt
`2ff8c05` (host_surface regen 238→239); mlopt `6018e10` (emit
sidecar), `96a2b7e` (audit-5 P5-N1/N2 fixes), `ca7a77f` (trainer
source-port + @opts parsing), `b4fb7f7` (`_census` memoization).
eqsat untouched this wave — skipped per instructions.

**WIP present and not disturbed:** graphopt has a large dirty
worktree (Cargo.lock, bench/discovery/runner.py, pyproject.toml, ~a
dozen test files — the repin agent's mid-flight work); mlopt
`pyproject.toml` is dirty.  All verdicts below are against committed
state; `cargo test`/`pytest` ran against worktrees as found.

## Per-commit verdicts

### source-onnx

- **`981c755` (bert family) — CLEAN.** Post-LN throughout matches HF
  BertLayer: attention block reads `x` directly (no pre-norm),
  `BertSelfOutput` = `LayerNorm(dense + attention-input)`
  (`arch.rs:2122-2126`), FFN reads the post-attention LN and closes
  with its own residual LN (`arch.rs:2136-2159`).  q/k/v scaled on
  the scores after the QK matmul (`arch.rs:2119-2121`) — HF's
  `1/sqrt(head_dim)` spelling, noted in-comment as differing from
  whisper's pre-scaled-q (both exact).  Leaf names are literal HF
  safetensors paths (`embeddings.*`, `encoder.layer.N.*`,
  `pooler.dense`) with `nn.Linear` `[out,in]` → `transpose_last_two`
  via the shared `linear()` helper (`arch.rs:3495`).  `token_type:
  "zeros"` binds row 0 of the table broadcast — the HF
  `token_type_ids=None` default; `"none"` and any other spelling
  refuse by name (`validate_bert`, `arch.rs:902-909`).  seq_len
  bounded by `max_position_embeddings`; position table sliced
  `0..seq` (`arch.rs:1993-2007`).  MiniLM-L6 spec dims
  (vocab 30522, d=384, 6 layers, 12 heads, ffn 1536, eps 1e-12)
  match `microsoft/MiniLM-L12-H384`'s L6 variant.
  `bert_minilm_binds_real_safetensors` **passed** — real checkpoint
  binding verifies every declared shape/dtype.
- **`208e713` (vit family) — CLEAN.** Conv2d patch embed
  `(D,C,p,p) stride=p` → flatten → transpose → CLS `expand`+`concat`
  → position add → embedding LN — HF's
  `patch_embeddings.flatten(2).transpose(1,2)` + `cls_token.expand`
  spelling (`arch.rs:2185-2245`).  seq = `grid²+1` computed from
  `image_size/patch_size` (`arch.rs:1826-1854`); input spatial dims
  must equal `image_size` — non-native resolutions refuse by name
  (position interpolation declared out of slice,
  `arch.rs:921-927`).  Pre-LN encoder layers, scores-scaled
  attention, `layernorm_before`/`layernorm_after` naming matches HF
  `vit.encoder.layer.N.*`.  `vit_base_binds_real_safetensors`
  **passed**.
- **`74b8dd6` (gpt2 family) — CLEAN.** The Conv1D transform split is
  correct and verified: every `nn.Conv1D` weight is `[in,out]` →
  `gpt2_linear` binds **identity** and `matmul` consumes directly
  (`arch.rs:2563-2587`); the fused `c_attn` projects to `3*d_model`
  then three `split` ops carve q/k/v along dim 2
  (`arch.rs:2490-2516`).  The only transpose is the tied
  `lm_head.weight`: `nn.Linear` `[vocab,d]` aliased to
  `transformer.wte.weight` via `weight_aliases`, term shape
  `[d,vocab]` post-transpose (`arch.rs:2428-2437`,
  `leaf()` alias rebind at `arch.rs:3379-3386`).  `gelu_new` spells
  HF's tanh approximation with the f64 literal
  `0.7978845608028654` (`arch.rs:2597-2610`).  wpe sliced `0..seq`;
  causal mask minted once, shared.  `gpt2_small_binds_real_safetensors`
  **passed**.
- **`9192c06` (llama family + primitives) — CLEAN.** Weight
  transforms correct: all projections bias-free `nn.Linear`
  `[out,in]` → `transpose_last_two` via `linear()`; RMSNorm leaves
  weight-only identity (`param_leaf`); `lm_head.weight` is a real
  checkpoint tensor (no alias needed, correctly commented
  `arch.rs:2649-2658`).  RoPE verified against HF
  `LlamaRotaryEmbedding`: `inv_freq = theta^(-2j/dh)` via
  arange/pow/reciprocal, `emb = cat(freqs,freqs)`, cos/sin reshaped
  `[1,1,T,dh]`; `apply_rotary_pos_emb` rotate-half
  `cat(-x2,x1)·sin + x·cos` (`arch.rs:2827-2905`).  GQA: k/v project
  at `num_key_value_heads*head_dim`, RoPE applied **pre-expansion**
  (HF order), `repeat_kv` = unsqueeze+expand+reshape with MHA
  identity shortcut (`arch.rs:2906-2943`, `tinyllama_mha_variant_
  skips_gqa_expansion` pins it).  Scores scaled by `head_dim^-0.5`
  after QK matmul; SwiGLU `silu(gate)*up → down_proj`
  (`arch.rs:2683-2708`).  Validation refuses odd head_dim (rotate-
  half), `heads % kv_heads != 0`, non-positive theta/eps
  (`arch.rs:1029-1090`).  Spec honestly documents the AWQ
  checkpoint can't bind float leaves — binding test env-gated, no
  fabricated dims anywhere (all dims spec-declared + validated).
- **`8c6dd79` (whisper combined + name-keyed payload) — CLEAN with
  one latent LOW (P6-N1).**
  - Combined spec: `input` = encoder frontend, `decoder_input` =
    token ids; `encoder_output` declared on a combined spec refuses
    by name; batch/d_model equality across sections validated
    (`arch.rs:471-505`, `validate_decoder(combined)`).  Encoder root
    term feeds cross-attention inline (`whisper_forward`,
    `arch.rs:2945-2964`; `begin_decoder` swaps the active
    `Section`'s weight prefix `model.encoder.*` → `model.decoder.*`
    — leaves can't cross-bind).
  - Name-keyed payload: `bind_weights` keys safetensors entries by
    **leaf name** (`arch.rs:1366-1420`), so whisper's tied
    `proj_out.weight`/`embed_tokens.weight` — one checkpoint tensor
    under identity *and* `transpose_last_two` — emits both copies
    honestly instead of refusing.  Every leaf still re-checked
    (dtype name, pre-transform shape) — binding implies
    verification; the manifest rewrite sets `transform:"identity"`,
    `source_shape`=post-transform shape, and `source`=leaf name =
    payload key (`examples/arch.rs:162-172`).  Alias sources absent
    from the payload are included raw so presence checks see the
    checkpoint's answer (`arch.rs:1405-1417`).
  - whisper-tiny-full spec dims check out: conv frontend
    `conv_out_len(3000,k3,p1,s1)=3000 → s2 → 1500` =
    `max_source_positions`; decoder_input `[1,32]` ≤
    `max_target_positions` 448; 168-leaf census pinned by test.
    `whisper_tiny_full_binds_real_safetensors` **passed**.
  - **P6-N1 (LOW):** the encoder `embed_positions.weight` leaf is
    the *full* `[max_source_positions, d_model]` table added
    unsliced to `x` of shape `[B, positions, D]`
    (`arch.rs:3009-3017`).  Validation refuses only
    `positions > max_source_positions` (`arch.rs:1703-1709`) — a
    spec whose conv frontend yields `positions <
    max_source_positions` assembles an ill-typed `add`
    (`[B,T,D] + [M,D]`, T≠M) rather than a named refusal.  Every
    other family's position table is sliced to the live length
    (decoder `0..target`, gpt2/bert `0..seq`).  Committed spec has
    positions == max → latent, unreachable today; recommend
    `positions != max_source_positions → Spec` refusal or a
    `0..positions` slice like the decoder's.

### host-tensor

- **`eec98bb` (tensor.load) — CLEAN.** `Load::call` enforces
  1–2 args, string path, string-or-null name (`codec.rs:281-296`).
  `parse_safetensors` is fully bounds-checked: 8-byte LE length,
  `checked_add` header bound, JSON object header, per-entry dtype/
  shape/`data_offsets` validation, `__metadata__` skipped, unnamed
  load requires exactly one tensor else names the keys, named miss
  names the file's keys, `data_offsets` bounds-checked against the
  payload slice (`codec.rs:555-660`).  dtype is an exact match —
  `"F64"` identity, `"F32"` widens, everything else a named error
  (`codec.rs:621-628`); byte length must equal `numel × elem_size`
  (`checked_mul`, overflow → error).  Shape comes from the header,
  never the call site — the contract holds.  Path resolution:
  absolute passthrough, relative → `$TENSOR_LOAD_DIR` (non-empty)
  else cwd (`codec.rs:300-311`) — matches the documented
  stdin-eval base-dir contract.  Non-finite elements ride f64
  natively and re-encode through `__nonfinite__` tags
  (`load_nonfinite_elements_tag_on_wire` pins it).  Registered
  across `TENSOR_OPS`/`index.hnl` cap+op+handle+export — the
  handwritten-surface drift guard is green.  `cargo test --lib`:
  **114 passed, 0 failed**.

### graphopt

- **`2ff8c05` (host_surface regen) — CLEAN.** `tensor.load` entry is
  `Value... -> Tensor`, `kind: handwritten` — matches the
  `index.hnl` decl verbatim.  Count bumped 238→239 consistently in
  JSON + test.  `pytest tests/test_host_surface.py`: **9 passed**.

### mlopt

- **`6018e10` (emit sidecar) — CLEAN.** Safetensors writer is
  format-exact for the `tensor.load` reader: `u64le header_len` +
  JSON `{name:{dtype,shape,data_offsets}}` + raw LE payload,
  payload-relative offsets, contiguous (`emit.py:2107-2145`);
  verified against the real `safetensors` python lib too (loads
  fine — see P6-N2 for the padding note).  `_Sidecar.expr` diverts
  only `Param` leaves ≥ `min_elems` (default 4096) with
  data-bearing feeds — `Var` leaves, `{"hnl"}` computed feeds, and
  sub-threshold params stay inline `from_list` (pinned by three
  tests).  Safetensors key = feed leaf name = the
  `tensor.load` second arg — consistent end to end
  (`emit.py:2158-2181`).  `F32` rounding is a deliberate,
  documented opt (`--sidecar-dtype`); `F64` is identity with
  non-finites native.  Default path writes nothing — hot verify
  callers untouched (pinned).  `KitRunner.tensor_load_dir` exports
  `TENSOR_LOAD_DIR` only when set (`emit.py:2466-2468`);
  `mlopt export --sidecar`/`--check`/`--verify-run` wire the
  sidecar's parent dir through, and verify-run re-emits the same
  deterministic `tensor.load` spelling so the gate exercises the
  shipped weight channel (`cli.py:319-336`).  Minor non-issues
  noted: `_leaf_array` doesn't cross-check `len(flat)==numel`, but
  a mismatch produces a sidecar `tensor.load` refuses by name at
  eval — honest downstream failure, never wrong values; a failed
  `--out` write can orphan the sidecar — cosmetic.
- **`96a2b7e` (audit-5 fixes) — CLEAN, closes P5-N1/N2.**
  `om_apply` unfold now requires `len(args)==1`, no attrs on both
  the `om_apply` and its `om_elem` child; `om_decayed` requires
  arity 3 + no attrs (`carrier_expand.py:66-85`) — non-canonical
  spellings pass through to emit's named decline.  `_permute`
  reunites trailing `argN` attrs with positional operand args by
  original position (the torch `permute(x, *dims)` spelling), lone
  list arg stays the list spelling, and non-integral floats now
  decline by name in both `_permute` and `_conv1d_dim`
  (`emit.py:595-650`, `:551-556`).  60 new test lines pin both.
- **`ca7a77f` (trainer fixes) — CLEAN.** `Optimizer(source=
  spec.source)` mirrors `optimize.py:171` exactly — `None` keeps
  `TorchBackend().source`, a pre-lifted `Source` port overrides;
  `call_kwargs` forwards `ModelSpec.kwargs` into every
  `DeliveryCase` benchmark call (`deliver.py:255-269` contract).
  `parse_ref` splits @opts on bracket-depth-0 commas — JSON
  `leaves={...}` values survive; unbalanced input degrades to a
  `ValueError`, never a silent misparsed opt.  Dev script, honest.
- **`b4fb7f7` (_census memoization) — CLEAN.** `_census` is now an
  iterative `id()`-memoized walk: `Op.make` interning makes equal
  subtrees the same object, so memoizing can't skip *distinct*
  nodes — every reachable `Var`/`Param` is still collected exactly
  once, and `reversed(args)` push preserves the old recursive
  preorder so `vars_` stays first-use-ordered
  (`onnx_rust.py:334-361`).  The 2²⁰-path regression pin covers the
  exponential blowup.  Iterative also fixes the recursion-depth
  hazard on deep transformer terms.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P6-N1 | LOW | `source-onnx/src/arch.rs:3009-3017` vs `:1703-1709` | Whisper encoder `embed_positions.weight` binds the full `[max_source_positions, d]` table and adds it unsliced; validation refuses only `positions > max`, so `positions < max` assembles an ill-typed `add` instead of a named refusal. Committed spec has positions == max — latent. Fix: refuse `!=` or slice `0..positions` like the decoder does. |
| P6-N2 | INFO | `mlopt/src/mlopt/emit.py:2138-2144` | `write_safetensors` doesn't pad the JSON header to the safetensors 8-byte convention. Verified harmless: `tensor.load`'s reader and the real `safetensors` python lib both accept the unpadded file (hlen 55, total 63 mod 8 = 7 → loads fine). Cosmetic spec-conformance note only. |
| P6-N3 | INFO | `mlopt/tests/test_emit.py:635-642` | `test_export_declines_when_load_fails` is environment-stale, unrelated to this wave: the kokoro adapter is dep-free by design (misaki/espeak stub-imported, `kokoro.py:91,583` — `available=True` unconditional) and its weights resolve in this env, so `export kokoro:…` now runs the full pipeline instead of declining at load. The test pins an environment assumption, not a contract — recommend repinning to a genuinely-unavailable adapter or skipping on env. |
| P6-N4 | INFO | `mlopt/src/mlopt/emit.py:2158-2181` | `_Sidecar.expr` records `(shape, flat)` without checking `len(flat) == prod(shape)`; a malformed feed writes a sidecar `tensor.load` refuses by name at eval ("payload N bytes != elements × elem") — honest failure downstream, never wrong values. |

No MEDIUM+ findings.

## Scorecard

| Invariant | Result |
|-----------|--------|
| Verified-or-declined | **Holds.** Every new family validates dims/divisibility/activation/transform before assembling; unsupported activations, token_type spellings, non-native ViT resolutions, AWQ-incompatible dtype claims all refuse by name. `bind_weights` re-checks dtype+pre-transform shape per leaf; `tensor.load` refuses bad dtype/shape/offsets/truncation by name; `--sidecar` declines surface as `export declined:` exit 1. |
| No fabricated data | **Holds.** All spec dims are declared + cross-validated (positions computed via `conv_out_len`, seq=grid²+1, head_dim=hidden/heads); AWQ checkpoint honestly documented as unbindable rather than approximated; `input_source`/`leaf_source` provenance unchanged; sidecar carries real param values only — `Var`/`{"hnl"}` never sidecar. |
| No silent fallback | **Holds.** Name-keyed payload emits both transforms of a tied tensor as separate honest entries (no source-key collision, no dropped claim); alias sources included raw for presence checks; `_census` memoization cannot skip nodes (id-interned DAG); P5-N1/N2 latent holes closed — non-canonical carriers and non-integer dims now decline by name. |
| Guards actually guard | **Holds.** cargo test source-onnx: lib 1 + arch 30 + weights 7 + artifacts 2 + liftmap_drift 1 + synthetic 12 — **all green**, including real-checkpoint binds for whisper-tiny-full, whisper-tiny-decoder, whisper-large-v3-turbo, gpt2-small, bert-minilm, vit-base. host-tensor `cargo test --lib`: 114 green. mlopt targeted pytest (emit/cli sidecar+permute+conv1d, onnx_rust/arch_rust/emit_surface): 87 passed, 1 env-stale failure (P6-N3, pre-existing). graphopt host_surface: 9 green. |

**Verdict: PASS** — all commits preserve the honesty invariants.
One new latent LOW (P6-N1, whisper encoder position-table
slice/refusal gap — unreachable with committed specs) and three
INFOs; P5-N1/N2 confirmed fixed.
