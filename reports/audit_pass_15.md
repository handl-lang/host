---
title: Audit pass 15 — kit-cuda ci.sh gate fix, graphopt layers restore, source-onnx streaming safetensors, D-1d still unlanded, eqsat tag drift
type: report
date: 2026-11-16
---

# Audit pass 15

Scope: committed state since pass 14 — kit-cuda `5be7310` (ci.sh
wires `lint-imports` + coverage floor, resolving P14-N1), graphopt
`935b061` (layers contract restores `kit_cuda` above `mlopt_torch`,
addressing P14-N2), source-onnx `bf71d5e` (streaming bound-payload
safetensors writes + header-only `WeightStore`), the in-flight
D-1d `mlopt-discovery → kit-discovery` cascade, source-onnx arch
wave-2 WIP (qwen2 + distilbert), and eqsat's untagged commits past
`v0.1.9`.  mlopt `da1dc25` skipped (clean in pass 14).

**WIP present and not disturbed:** graphopt has a ~400-file
uncommitted D-1d cascade (`packages/mlopt-discovery` deleted,
pyproject repinned to a `kit-discovery` git dep,
`gamekit`/`discoverykit` tag bumps); kit-discovery has the same
139-file uncommitted rename as pass 14; source-onnx has uncommitted
`src/arch.rs` +755 / `tests/arch.rs` +550 plus two new spec files.
A stale `graphopt` stash (`stash@{0}`, decode-step fusion probe)
also exists — unrelated, noted only.

## Per-commit verdicts

### kit-cuda `5be7310` — CLEAN; P14-N1 resolved and verified live

- `ci.sh` now runs `.venv/bin/lint-imports` then
  `coverage run -m pytest tests/ -q && coverage report -m`
  (`ci.sh:9-11`) — the exact kit-carriers `6265f96` pattern.
- **Ran the gate:** `.venv/bin/lint-imports` → "kit_cuda is the cuda
  top … KEPT" (8 files, 7 dependencies analyzed).  `fail_under = 99`
  still declared (`pyproject.toml:99`, `source = ["kit_cuda"]`).
  Worktree clean at `5be7310`.

### graphopt `935b061` — CLEAN at committed state; contract is honest

- Committed layers (`git show HEAD:pyproject.toml:201-214`):
  `mlopt_discovery > kit_cuda > mlopt_torch > mlopt_dtypes`,
  `root_packages` still includes `mlopt_discovery`, and
  `include_external_packages = true`.
- **Upward-edge ban is real:** at HEAD the only `kit_cuda` imports
  in the repo are lazy in-function binds in
  `mlopt_discovery/play/gen.py:657` and
  `mlopt_discovery/play/torch_arm.py:258` — both *above* `kit_cuda`
  in the layering, so the edges are legal.  `mlopt_torch`'s sole
  `kit_cuda` mention is inside the `runners.py:1-19` module
  docstring (a "do not re-export, it would cycle" note) — grimp
  sees no edge, so `mlopt_torch ↛ kit_cuda` is genuinely enforced
  now, closing the P14-N2 hole where the split had dropped the
  entry entirely.
- `kit_cuda` cannot legitimately sit lower: kit-cuda's own imports
  are `torch`, `mlopt_core.failures`, `mlopt_torch.device`,
  `mlopt_torch.device_placement` only (pass-14 verified; its
  import-linter contract KEPT again this pass).
- **Caveat on verification depth:** `lint-imports` could not be
  executed at committed HEAD because the worktree is mid-D-1d
  cascade — the installed `mlopt-discovery` editable path is
  deleted and the worktree pyproject already points at the
  not-yet-existent `kit-discovery` git repo, so the venv's module
  graph is broken independent of this commit.  The verdict rests
  on static graph analysis (imports enumerated above), which is
  unambiguous here.

### source-onnx `bf71d5e` — CLEAN; parity claim verified by re-running it

- **Re-ran the parity test:** `cargo test --test stream_parity` →
  **3 passed** (whisper-tiny, t5-small, gpt2-small — all three HF
  checkpoints present locally, so nothing skipped; ~8 s).  The test
  (`tests/stream_parity.rs:36-63`) compares **every byte** of the
  streamed file against `bind_weights`' in-memory `bound.bytes`,
  chunked at 4 MiB with a final length assert — a real byte-parity
  check, not a hash or spot check.
- **Header replication verified against the crate source**
  (safetensors 0.4.5, `Cargo.lock:1119`): `prepare`
  (`tensor.rs:167-208`) sorts by `right.dtype().cmp(&left.dtype())
  .then(name)` — descending `Dtype` enum order (= alignment order,
  `tensor.rs:654` "MUST be in increasing alignment order") then
  name; the hand-rolled `order.sort_by` in `write_bound_weights`
  (`src/arch.rs`, new code ~line 1960) uses the identical
  comparator.  `Metadata::serialize` (`tensor.rs:455-479`) emits
  map entries in `data_offsets` order via `serialize_map`, and
  `TensorInfo` is a derived `Serialize` struct
  (`dtype`,`shape`,`data_offsets` field order, tuple → JSON array)
  — the streaming code serializes the *same* `TensorInfo` struct
  and writes `"name":{...}` pairs in `order` sequence by hand,
  correctly avoiding `serde_json::Value`'s BTreeMap key-sort.  The
  8-byte space-padding formula `(8 - len % 8) % 8` matches
  `tensor.rs:199-201`, and `data_offsets` are relative to the
  post-header buffer in both.  `apply_transform`'s post-transform
  shape (`src/arch.rs:2414`) is identical to the new
  `transformed_shape` helper (same `swap(n-2, n-1)`).
- **`weights.rs` rewrite is careful:** `tensor()` now reads only
  the tensor's `data_offsets` range via `read_exact_at` (unix) /
  seek+read (elsewhere), with `checked_*` offset arithmetic,
  inverted-range rejection, and a file-size bound before
  allocating (`src/weights.rs`, `tensor()` ~line 340).  The old
  path's `SafeTensors::deserialize` whole-file validation is
  replaced by `TensorView::new(dtype, shape, &buf)` — which
  re-validates byte-length vs shape×elem — plus the new explicit
  bounds checks; malformed-header behavior changes from a
  safetensors error to named `io::Error`/`WeightError`, same
  exception families.
- **Residual INFO:** all three parity checkpoints are single-dtype
  float32 payloads, so the *mixed-dtype* ordering branch — the one
  place the hand-rolled sort could actually diverge from
  `serialize` — is verified by source-reading, not exercised.  The
  comment itself flags this case ("an alias carrying a checkpoint
  dtype different from the spec's `weight_dtype`").  A future
  bf16/fp8-mixed spec would be the first real test of it.

### D-1d kit-discovery — still unlanded; the consumer cascade is now ahead of it

- kit-discovery HEAD is still `1d6429f` (the pre-split transplant):
  committed `pyproject.toml` still reads `name = "mlopt-discovery"`,
  `version = "0.1.0dev"`.  The 139-file `RM` rename +
  `src/kit_discovery/*` rewrite remains **uncommitted**, and the
  worktree pyproject still carries **`fail_under = 0`**
  (`pyproject.toml:108`) — the placeholder floor persists.
- **No `origin` remote** — only the local-path `graphopt` remote.
  `git ls-remote https://github.com/handl-lang/kit-discovery` →
  **Repository not found**: the repo does not exist on GitHub yet.
- **New this pass — the risk is now concrete:** graphopt's
  uncommitted cascade already repins the product dep to
  `kit-discovery = { git = "https://github.com/handl-lang/
  kit-discovery.git", tag = "v0.1.0" }` (worktree
  `pyproject.toml` `[tool.uv.sources]`).  If that cascade commits
  before the kit-discovery repo is created, tagged, and pushed,
  `uv sync` breaks for every consumer.  Ordering matters: land
  kit-discovery (commit → push → `v0.1.0` tag → real
  `fail_under`) *before* the graphopt/mlopt/kernelopt cascade.

### arch wave-2 WIP (source-onnx, uncommitted) — specs structurally sound

- `data/architectures/qwen2-0.5b-instruct.json` and
  `distilbert-base-uncased.json` are valid `mlopt.arch/v1` docs with
  coherent dims (qwen2: hidden 896 / 24 layers / 14q·2kv GQA /
  intermediate 4864 / vocab 151936 / `attention_bias: true` /
  `tie_word_embeddings` alias `lm_head.weight →
  model.embed_tokens.weight`; distilbert: 6 post-LN layers /
  768·12·3072 / LN eps 1e-12 / `gelu`).  Descriptions are honest —
  the qwen2 spec explicitly notes the only local checkpoint is the
  AWQ int4 sibling whose qweight/scales tensors can't bind float
  leaves, so its binding test is env-gated.  The +755-line
  `src/arch.rs` diff adds `Qwen2Spec`/`DistilbertSpec` serde
  structs, family dispatch, and emitters reusing the llama/bert
  paths — consistent with the spec docs.  Committed state
  unaffected; not audited line-by-line (WIP).

### eqsat tag drift — real but minor; a v0.1.10 is worth it, not urgent

- Commits past `v0.1.9`: `1b4ed13` (mlopt_cuda→kit_cuda contract
  rename), `cdc0771`/`a4fe2f9`/`0bb5940` (three law-corpus snapshot
  regens: 468→488→506→524 records + prior_art port pins),
  `4e2dc87` (doc refs).  `git describe` → `v0.1.9-5-g4e2dc87`.
- Consumers (graphopt/mlopt/kernelopt/kit-discovery worktree) all
  pin `tag = "v0.1.9"` on the `packages/mlopt-*` subdirectories.
- **Functional impact ≈ nil for the rename:** `1b4ed13` touches
  only docstrings, the import-linter `forbidden_modules` list, and
  `test_backend_neutral` `_FORBIDDEN`/`_BLOCKED` — no shipped code.
  And the root `pyproject.toml` contract isn't part of the
  subdirectory packages consumers install, so the stale
  `mlopt_cuda` forbidden-name at v0.1.9 guards nothing anyone
  runs.  Dead-name forbiddens are harmless; `kit_cuda` being
  unlisted at v0.1.9 only matters if someone ran eqsat's own gates
  *at that tag* — eqsat HEAD is already correct.
- **The real (small) gap is the law regens:** three commits of new
  minted-law corpus + prior_art pins are unreachable to every
  pinned consumer until the next eqsat tag.  Recommend tagging
  `v0.1.10` at the next natural release point so the D-1c rename
  and the law updates ship together — bookkeeping, not a defect.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P15-N1 | MEDIUM | kit-discovery (whole repo) + graphopt worktree `pyproject.toml` `[tool.uv.sources]` | D-1d is still entirely uncommitted (139 `RM` files, `fail_under = 0` placeholder at `pyproject.toml:108`), the GitHub repo does not exist (`ls-remote` 404), and there is no `origin` remote — yet the in-flight graphopt cascade already repins consumers to `kit-discovery@git…@v0.1.0`. Commit order matters: the split repo must land and be tagged before the consumer cascade, or `uv sync` breaks workspace-wide. |
| P15-N2 | LOW | eqsat `v0.1.9` vs HEAD (`1b4ed13`, `cdc0771`, `a4fe2f9`, `0bb5940`, `4e2dc87`) | Tag drift: pinned consumers get the pre-rename contract (dead `mlopt_cuda` entry, no `kit_cuda` guard — harmless since the contract isn't shipped via subdirectory pins) and miss three law-corpus regen commits. Worth a `v0.1.10` at next release; no functional gap today. |
| P15-N3 | INFO | source-onnx `tests/stream_parity.rs` + `src/arch.rs` `write_bound_weights` | Parity claim fully verified — re-ran green (3/3, byte-exact chunked compare vs `bind_weights`), and the hand-rolled safetensors header matches crate source 0.4.5 line-for-line (sort comparator, `TensorInfo` field order, padding, offset base). Only untested branch: mixed-dtype payloads (all three checkpoints are f32) — the alias-with-foreign-dtype case the code comment itself flags. |
| P15-N4 | INFO | graphopt worktree `pyproject.toml` (uncommitted) | The WIP layers table drops `mlopt_discovery` from `root_packages`/layers but keeps `"kit_cuda"` in `layers` — with no root package importing `kit_cuda` anymore (the lazy imports moved to `kit_discovery`, which left the layers table), `lint-imports` currently fails "Missing layer 'kit_cuda'". Expected to be resolved when the cascade lands (either `kit_discovery` rejoins as a layer or `kit_cuda` leaves the table) — flagged so it isn't committed red. |
| P15-N5 | INFO | source-onnx `src/weights.rs` `tensor()` | The `SafeTensors::deserialize` whole-file validation is replaced by manual offset bounds + `TensorView::new`; equivalent-or-stronger on real files, but malformed-header error types change (named `io::Error`/`WeightError` instead of safetensors errors). One `File::open` per tensor lookup — fine at these sizes. |

No HIGH findings.  P14-N1 is resolved and verified live
(`lint-imports` KEPT, coverage floor wired into `ci.sh`);
P14-N2's committed-state half is resolved honestly (the
`kit_cuda`-above-`mlopt_torch` layer is real and the only
`kit_cuda` consumers sit above it — the residual is the
uncommitted-cascade lint-imports breakage in P15-N4, which is
in-flight work, not committed state).  The streaming-safetensors
parity claim is the strongest kind: independently re-run and
green, with the byte-layout replication confirmed against the
safetensors crate source rather than trusted.  The one item
needing action is ordering, not code: kit-discovery must exist on
GitHub with a `v0.1.0` tag and a real coverage floor before the
graphopt/mlopt/kernelopt consumer cascade commits.

**Verdict: PASS with one MEDIUM** — everything committed since
pass 14 checks out; the MEDIUM is a landing-order risk in the
in-flight D-1d split (repo doesn't exist yet while consumers
already point at it), plus the standing `fail_under = 0`
placeholder that must become a measured floor before the split
ships.
