---
title: Audit pass 17 — dtype_ctx creator-dtype fix audited clean, verify-run payload tolerance verified coarser-only, qwen2 bf16 fix landed, but v0.1.17 pin drift leaves mlopt's venv a hand-patched frankenstein
type: report
date: 2026-11-16
---

# Audit pass 17

Scope: committed state since pass 16 — graphopt `3e724a8`
(mlopt-torch `dtype_ctx` creator-dtype inheritance), mlopt `8d5f8bb`
(verify-run payload-dtype tolerance), source-onnx `0210522` +
`0a4571c` (qwen2 bf16 fix + rope_tables doc), graphopt `v0.1.17`,
the D-1d `kit-discovery` split state, and host-tensor `tensor.load`
streaming (in-flight agent; committed state only).

**WIP present and not disturbed:** graphopt still carries the ~400-file
uncommitted D-1d cascade (`packages/mlopt-discovery` deleted,
pyproject repinned); mlopt has the matching 9-file cascade plus four
untracked run artifacts; kernelopt has modified files incl. the same
repin; kit-discovery has 237 staged-but-uncommitted rename files
(111 `A` + 123 `R` + 2 `D` + 1 `M`, HEAD still `1d6429f`);
host-tensor has one uncommitted file (`src/ops/codec.rs`, +160/−33 —
the streaming `tensor.load` rewrite, audited only as far as "present
and uncommitted").

## Per-commit verdicts

### graphopt `3e724a8` — CLEAN; all four audit questions verified

**(a) `LIKE_CTX` reset discipline — correct on every path.**
`dtype_ctx.py:33-36` is a plain `ContextVar` (default `None`).
Both set sites wrap the entire eval in `try/finally` with
`LIKE_CTX.reset(ctx_tok)`:

- `_EvalPlan._run_tail` — `torch_eval.py:291` set, `:326` reset in
  `finally`. Covers every step of the tape including the liveness
  release loop; an exception mid-tape still resets.
- `eval_term` — `torch_eval.py:583` set, `:587` reset in `finally`,
  around the whole `go(term, go)` recursion.

Nested evals are safe by construction: an inner `eval_term` /
`_run_tail` (e.g. an `opaque` binding that re-enters eval, or a
nested `IRModule` forward) sets its own token and resets to the
*outer* value on exit — ContextVar tokens restore, they don't clear.
Threads each get their own context; the var is only read on the
calling thread inside bindings, and torch async/CUDA-graph capture
never re-reads it. Outside any eval window `like_dtype()` returns
`None` → torch ambient default, the historical behavior
(`dtype_ctx.py:40-55`; pinned by `test_creator_dtype_ctx.py:74`).

**(b) Explicit `dtype` attr still wins everywhere.** Both new
helpers check `_creator_dtype(kw)` first and only fall back to
`dtype_ctx.like_dtype()` when it returns `None`:
`_creator_dtype_ctx` (`torch_bindings.py:2588-2600`),
`_arange_torch` (`:470-476`), `_full_torch` (`:489-497`). The
`*_like`/`new_*`/`to` bindings deliberately keep plain
`_creator_dtype` so operand-dtype inheritance is untouched
(`:1935-1965`, `:378-392`).

**(c) Creator coverage is complete for the shipped table.** Every
no-operand float creator in `_CORE_TORCH_BINDINGS` now routes
through the ctx fallback: `linspace` (`:1689-1694`), `zeros`/`ones`/
`empty`/`randn`/`rand` (`:1909-1933`), `arange` (`:465-476`),
`full` (`:480-497`, now `_full_torch`). There are **no** other
ambient-default creators in the table — no `eye`, `randint`,
`normal`, `uniform`, `randperm`, `meshgrid`, `tensor`,
`empty_strided`, or `scalar_tensor` bindings exist (grepped the
full binding map and the attr tables). Int spellings are preserved:
int-bound `arange` (`any(isinstance(v, float))` gate, `:472`) and
int-fill `full` (`isinstance(fill, float)`, `:490`) keep torch's
int64 inference; `like_dtype()` itself refuses non-float/complex
likes so an int computation can't drag creators to float
(`dtype_ctx.py:48-54`). One residual nit (INFO): the ctx carries
*dtype only* — creators still resolve device via
`_creator_device(kw)` → ambient default device when no `device`
attr is recorded (`:2582-2585`). That is pre-existing behavior,
unchanged by this commit, and the verify path is CPU; a CUDA eval
with unpinned creators would already have been broken before.

`like` provenance is sound: `_run_tail` takes the caller's like,
falls back to first floating Param for param-only roots
(`torch_eval.py:271-276`), and `eval_term` computes its own via
`_const_like` (`:502`) — the same tensor `_mint_const` uses
(`:358-384`), so Const scalars and creators can never disagree.

**(d) Committed state is self-consistent.** `git status` shows the
three dtype_ctx files clean at `3e724a8`; the only mlopt-torch worktree
mods are the unrelated 4-line D-1d import renames in `region_fuse.py`/
`rl.py`. Tests: `pytest tests/test_creator_dtype_ctx.py` → **8/8
pass** locally (float arange f64/f32, int arange, explicit-dtype
wins, outside-eval ambient, all-creator sweep, full int/float fill,
ctx reset). No HIGH/LOW findings.

### mlopt `8d5f8bb` — CLEAN; coarser-of-two-bounds verified

`_verify_run_tolerance(dtype, payload_dtype="F64")`
(`cli.py:305-358`):

- **Can never tighten below the old bound.** The payload arm is
  `atol, rtol = max(atol, ptol), max(rtol, ptol)` (`:353`) — pure
  widening. f64-ref + F32-payload goes 1e-9 → ~6.1e-5; f32-ref +
  F32-payload stays ~6.1e-5 (max of equal values); F64/no-sidecar
  is byte-identical to the old function.
- **`sidecar=None` handled** (`cli.py:647-652`): payload collapses
  to `"F64"` because inline `from_list` literals are f64 — the
  `--sidecar-dtype` flag alone widens nothing, matching the commit
  message and pinned by `test_cli.py:158-163`.
- **F64 sidecar keeps the tight gate**: `payload_dtype == "F32"` is
  the only widening trigger (`:351`); `"F64"` falls through to the
  reference-dtype bound unchanged (`test_cli.py:158`).
- **Non-float ref + F32 payload** widens the exact compare —
  deliberate and documented in the test (`test_cli.py:170-176`:
  "quantized weights can move an integer-valued output off the
  exact value"). Honest direction.
- **Report line names everything**: `ref=… dtype=… payload=…
  atol=… rtol=… — <rationale>` (`:654-659`), rationale appends the
  payload clause (`:354-357`). Loud, never silent.

INFO nits: (i) the `payload_dtype == "F32"` string check means a
programmatic caller passing e.g. `"F16"` would get no widening —
unreachable via CLI (`choices=["F64","F32"]`, `cli.py:784-789`) and
`_Sidecar` validates dtype upstream; (ii) `payload_dtype` is derived
from the *flag*, not the file — if `--sidecar` were passed but emit
wrote no file, `cli.py:193-202` already nulls `sidecar`, so the
`sidecar is not None` guard covers it.

### source-onnx `0210522` + `0a4571c` — CLEAN; P16-N2 resolved

- `data/architectures/qwen2-0.5b-instruct.json:16` now
  `"weight_dtype": "bfloat16"` — matches the dense
  `Qwen/Qwen2.5-0.5B-Instruct` config (pass-16 verified bf16).
- `tests/arch.rs` fabricated-bind assert updated to
  `Dtype::BF16` (`:2362`); the AWQ-sibling refusal assertion widened
  to accept a `dtype`-named mismatch (`:2395-2401`) — honest, since
  the AWQ checkpoint mixes int4-packed projections with an F16
  embedding table and the first mismatch met may be either.
- `src/arch.rs:4160-4164` documents why `rope_tables`' `arange`
  carries no `dtype` attr — verified the claim against host-tensor:
  `k_arange` (`host-tensor/src/ops/aten_kernels.rs:2999-3027`)
  always emits the dense-f64 wire (`Tensor.data: Vec<f64>`), and
  torch's `dtype_ctx` fallback now mints f64 under an f64 eval —
  the two sides genuinely agree through the unpinned spelling, and
  an explicit pin would indeed force one side's dtype onto the
  other. Comment-only change, accurate.
- `cargo test --test arch qwen2` → **7/7 pass**; `distilbert` →
  **4/4 pass**; worktree clean. P16-N2 closed.

### graphopt `v0.1.17` pin drift — MEDIUM (new finding P17-N1)

`v0.1.17` = `3e724a8` (the dtype_ctx commit), `v0.1.16` =
`869c336` — i.e. **v0.1.16 predates not only dtype_ctx but also
`935b061` (the kit_cuda layer-ordering restore, audit-14 P14-N2)
and `e2a4e21` (radon re-baseline)**. Pass 16's "v0.1.16 = HEAD
935b061" was stale; the tag sits three commits back.

Consumers: mlopt `pyproject.toml:64,69` and kernelopt
`pyproject.toml:52` pin `mlopt-torch`/`mlopt-host-tensor` at
`v0.1.16` — **neither committed nor in the uncommitted D-1d
cascade does any pin move to v0.1.17**. kit-cuda pins `v0.1.15`
(`pyproject.toml:29`), kit-carriers `v0.1.14` (`:25`) — older
still, but those are their committed released states and neither
consumes the verify-run path.

**The drift is not benign — it is already load-bearing.** mlopt's
`.venv` `mlopt_torch-0.1.0.dev0.dist-info/direct_url.json` records
`commit_id 869c336, requested_revision v0.1.16`, yet
`dtype_ctx.py`, `torch_bindings.py`, and `torch_eval.py` in
site-packages are **byte-identical to graphopt's `3e724a8`
worktree** (verified by `diff -q` — the "copied files into the
non-editable venv" workaround). So the venv is a frankenstein:
metadata says v0.1.16, contents are v0.1.17+. The next `uv sync`
reverts mlopt-torch to `869c336` — **dtype_ctx disappears** —
while mlopt `8d5f8bb` stays committed. Consequence: the
verify-run dtype fix `8d5f8bb` was measured against (tinyllama
max_abs 4.53e-07 → 6.13e-14) is not reproducible from committed
pins. Failure direction is loud, not silent: without dtype_ctx an
F64-sidecar verify-run on creator-op models fails at ~4.5e-7 vs
the 1e-9 bound; the new F32-payload bound (6.1e-5) would mask it —
but 4.5e-7 is honestly inside the f32 tolerance anyway, so the
masking is legitimate, not a hole. Still, a committed feature's
measured behavior depending on hand-copied venv files is exactly
the silent-drift pattern the audits keep flagging. Fix is a
one-line pin bump (`v0.1.16 → v0.1.17`) on the two mlopt-torch/
host-tensor lines in mlopt + kernelopt — can ride the D-1d cascade
or a small deps commit.

### D-1d kit-discovery — MEDIUM; unchanged since pass 16 (P16-N1 stands)

- `git ls-remote origin` (git@github.com:handl-lang/kit-discovery.git)
  returns **rc=0, zero refs** — the GitHub repo is still empty.
- No tags (`git tag -l` empty), HEAD `1d6429f`, 237 files staged
  but uncommitted.
- All three consumer worktrees pin `kit-discovery @ v0.1.0`
  (graphopt `pyproject.toml:147`, mlopt diff hunk, kernelopt
  `pyproject.toml:57`) — none committed; `uv sync` remains broken
  workspace-wide (pass-16 verified `couldn't find remote ref
  refs/tags/v0.1.0`; state identical, not re-run).
- `fail_under = 82` (`kit-discovery/pyproject.toml:118`) still
  unverified by a coverage run — P16-N7 stands.
- kit-discovery's own `pyproject.toml:29` pins `mlopt-torch @
  graphopt v0.1.16` — when it eventually commits, it inherits the
  same pre-dtype_ctx pin; worth bumping to v0.1.17 in the same
  pass.

### host-tensor `tensor.load` streaming — WIP only, committed state untouched

HEAD `54d1f7e` (2026-11-15, pre-pass-16 window), `main` in sync
with `origin/main`, stash empty. The only change is the uncommitted
`src/ops/codec.rs` rewrite (+160/−33): `load_safetensors` now reads
only the 8-byte length prefix + JSON header + the requested
tensor's `data_offsets` range via `Seek` — the multi-GB
`std::fs::read`-per-call transient is gone, with sane guards
(header-length `checked_add` + `<= file_len` before allocating,
`data_offsets` bounds check against `file_len - hend`). Skimmed,
not audited — agent 2b090c61 still in flight. Nothing committed to
report.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P17-N1 | MEDIUM | mlopt/kernelopt `pyproject.toml` pins (`v0.1.16`) vs graphopt `v0.1.17`=`3e724a8` | No consumer pin includes the dtype_ctx fix; mlopt's venv already papers over it by hand-copied files (dist-info still says `869c336`/`v0.1.16`). `uv sync` reverts mlopt-torch and silently removes the fix `8d5f8bb` was verified against — loud FAIL on F64-sidecar creator-op models, masked-but-honest on F32. Bump pins to `v0.1.17` (rides D-1d cascade or a deps commit). |
| P17-N2 | MEDIUM | kit-discovery repo + 3 consumer worktrees | Unchanged from P16-N1: repo empty (zero refs), no `v0.1.0` tag, 237 staged files uncommitted, HEAD `1d6429f`; all consumer cascades uncommitted; `uv sync` broken workspace-wide. Land order unchanged: commit → push → tag → real floor → cascades. |
| P17-N3 | INFO | graphopt `torch_bindings.py:2582-2585` + `dtype_ctx` | `LIKE_CTX` carries dtype only, not device — unpinned creators still mint on the ambient device. Pre-existing, unchanged, CPU verify path unaffected; a CUDA eval would need a `device` attr or a ctx extension. |
| P17-N4 | INFO | mlopt `cli.py:351` | `payload_dtype == "F32"` exact-match — a hypothetical non-F64/F32 payload string would widen nothing. Unreachable via CLI choices; `_Sidecar` validates upstream. |
| P17-N5 | INFO | kit-discovery `pyproject.toml:29,118` | Its own pins (`mlopt-torch v0.1.16`, pre-dtype_ctx) and `fail_under = 82` both need attention when the split finally commits — bump torch pin to v0.1.17, run the coverage floor once. |

No HIGH findings. `3e724a8` survives the closest reading: the
ContextVar is reset in `finally` at both set sites, nested evals
restore correctly via tokens, threads are isolated, explicit dtype
attrs win in all three resolution paths, and the creator census is
complete for the shipped binding table. `8d5f8bb` is
coarser-only by construction (`max()`), handles `sidecar=None`,
and keeps the F64 gate tight. The qwen2 bf16 fix is verified
against the real spec and tests pass. The one new action item is
the v0.1.17 pin bump — small, but until it lands mlopt's
verify-run fix exists only in a hand-patched venv.

**Verdict: PASS with two MEDIUMs** — all new committed code audits
clean; the MEDIUMs are both pin/repo-state issues (v0.1.17 not
consumed; kit-discovery still unlanded), not code defects.
