---
title: Audit pass 12 — kit-onnx gates + key_prefix injectivity + T-101 emit fix + WP2 prior_art tail + D-1b kit-carriers split + gpt2 scale-profile fix
type: report
date: 2026-11-16
---

# Audit pass 12

Scope: committed state since pass 11 — kit-onnx `6b1766c` (coverage
floor + import-linter), source-onnx `7f4d145` (`key_prefix`
injectivity) + `edb8a75` (T-101 emit fix), the WP2 prior_art port
wave (engine-eqsat `c886f9f`/`fe6353d`/`d9b7a41`, kit-eqsat
`799d3e4`/`58d74ad`/`0cbd0bf`, eqsat `3046ed2`/`7e24d7d`/`a4fe2f9` —
corpus now **488** records, not the 468 the scope note cites), the
D-1b carriers split (kit-carriers `e121a72`/`e192824`, graphopt
`1ab7e43`, eqsat `414ec4e`), and the gpt2-scale profile follow-up
(engine-eqsat `3443774` + egraph `2b83e74`).

**WIP present and not disturbed:** engine-eqsat `src/builtin.rs` +
`tests/contracts.rs` dirty (the §1 matmul/linear/conv-merge block,
prior_art 135→153 — consistent direction, corpus not yet regen'd);
kit-eqsat `prior_art.json`/`lib.rs`/`corpus_drift.rs`/docs dirty
(same slice, kit side); kit-carriers `pyproject.toml` dirty
(`ty>=` → `ty==`) + untracked `uv.lock`.  Committed sides verified
consistent: kit-eqsat 135-record prior_art pack = engine-eqsat
committed `prior_art()` (contracts.rs pins 135); eqsat committed
`eqsat_laws.json` = 488; parity suite green at HEAD.

## Per-commit verdicts

### kit-onnx `6b1766c` — CLEAN, both gates verified live

- **Import-linter contract actually bites.** Scratch-verified (files
  created, tested, deleted): a `kit_onnx/_scratch_violation.py`
  containing `import torch` breaks the contract
  (`kit_onnx._scratch_violation -> torch`); a *transitive* violation
  (`kit_onnx._scratch_violation -> kit_onnx._scratch_helper -> torch`)
  is also caught — the contract scans every `kit_onnx` module's
  imports, not just the entry point.  `forbidden` covers
  `torch`/`mlopt_torch`/`mlopt_carriers`/`mlopt_orchestrator`/
  `mlopt_host_tensor`/`mlopt_cuda` with
  `include_external_packages = true`
  (`pyproject.toml:107-121`).  `.venv/bin/lint-imports` on the clean
  tree: 1 kept, 0 broken.  Residual (INFO): the contract names a
  fixed module list — a *new* forbidden dep (e.g. a future
  `mlopt_backend_x`) needs a manual list edit; acceptable for a
  leaf contract.
- **Coverage floor is honest and enforced.** Re-ran the exact ci.sh
  tail: `coverage run -m pytest tests/ -q` (41 passed, 3.4 s) +
  `coverage report` → **TOTAL 43%**, exit 0 against
  `fail_under = 43` (`pyproject.toml:82`).  The 43.12% claim in the
  commit message matches (display rounds to 43%).  Margin is thin
  by design — ratchet convention, tighten-never-lower — and the
  floor sits on `[tool.coverage.run] source = ["kit_onnx"]` so
  graphopt-side exercise can't inflate it.  `ci.sh:10-14` wires
  `lint-imports` + the coverage run/report; `uv sync --group dev`
  installs `import-linter>=2.15`.
- **P11-N1 confirmed fixed** (graphopt `28b8b14`): ci.yml
  `--source` now `mlopt_torch,mlopt_cuda,mlopt_discovery` —
  `kit_onnx` dropped from both the run source and (implicitly) the
  report; the kit-onnx-side floor above is the replacement wall.

### source-onnx `7f4d145` (key_prefix injectivity) — CLEAN

`ArchAssembly::validate_key_prefix_injectivity` (`src/arch.rs:1650`)
runs at the end of `assemble()` (`:1989`) over **every**
checkpoint-bound path: `self.leaves[].source` and
`self.aliases[].source`.  Cross-checked against all
`checkpoint_key` call sites — `verify_weights` leaf loop (`:1714`)
+ alias loop (`:1737`), `bind_weights` leaf loop (`:1777`) + alias
loop (`:1817`) — there is no fifth lookup path; leaf `name`s and
manifest/`leaves_json` output correctly stay logical.  The
`leaf()` alias-resolution at `:4251-4257` rewrites a leaf's source
through `weight_aliases[].name → .source` *before* the leaf is
pushed, so the checked `leaf.source` is always the post-resolution
checkpoint path — no bypass.

Correctness cases walked:

- **Identical logical sources sharing a key stay legal** — the
  check refuses only when `prev != logical` (`:1657-1658`); gpt2's
  tied `lm_head.weight`/`transformer.wte.weight` pair (same source
  string) passes, pinned by `key_prefix_injective_remap_assembles`.
- **Distinct-logical collisions refuse by name** — both spellings
  covered by `key_prefix_colliding_sources_refuse` (strip-side and
  add-side); error names both sources + the checkpoint key.
- **Strip→empty edge**: `checkpoint_key("transformer.")` under
  `strip="transformer."` yields `""` (or `add` alone).  A *single*
  such source produces key `""`/`"<add>"` — no collision detected,
  but `verify_weights`/`bind_weights` then fail loudly on the
  missing tensor (named `Bind`/`Missing` error).  Two distinct
  sources both stripping to `""` collide on key `""` and refuse.
  No silent path; INFO at most.
- **Non-prefix strip**: strip is prefix-only (`strip_prefix`,
  `:1629`), so `"xtransformer.foo"` doesn't strip — correct.
- `cargo test --test arch` — **44 passed** incl. the env-gated real
  safetensors binds; no committed spec relies on a now-refused
  collision.

### source-onnx `edb8a75` (T-101 emit fix) — CLEAN; the
### "over-strict for decoder-only" worry is moot

Re-derived the bounds against `t5_position_bias`
(`src/arch.rs:4114-4215`):

- `eff_buckets = num_buckets/2` iff `bidirectional` (encoder),
  `num_buckets` otherwise (decoder); `max_exact = eff_buckets/2`
  (`:4142-4168`).  Encoder `num_buckets < 4` → `eff ≤ 1` →
  `max_exact == 0` → `div rel_f / 0.0` + `is_small` false
  everywhere → the `where` dead-branch protection the clamp relies
  on is gone.  The `< 4` refusal (`:1392`) is exactly the encoder
  bound.
- **There is no decoder-only T5 emission path** — `family: "t5"`
  always emits encoder+decoder in one `t5_forward` (`:3852`,
  `enc_bias` `:3874` + `dec_bias` `:3886`); `encoder_output` is
  refused for t5 (`:1345`); whisper decoder uses learned positions,
  gpt2/llama have no relative buckets.  `relative_attention_*`
  fields exist only on `T5Spec`.  So `num_buckets ∈ {2,3}` — valid
  for a hypothetical decoder-only — can never reach the emitter;
  every spec that hits the refusal previously produced a term that
  could only fail at eval (div-by-zero → NaN → loud `tensor.to`
  cast error).  The refusal is strictly better, not over-strict.
- `max_distance <= num_buckets/2` (`:1410`): decoder `max_exact =
  num_buckets/2` (integer division, odd `num_buckets` included —
  `5/2 = 2` both sides), encoder's is smaller, so the decoder bound
  is the binding one, as the comment claims.  `==` → `denom =
  ln(1) = 0` → `0/0` NaN on elements `where` *observes* (the clamp
  only rewrites dead elements) — refusal correct; `<` → negative
  `rel_if_large` → buckets out of range — refusal correct.  Bound
  is tight: `max_distance = num_buckets/2 + 1` passes and gives
  `denom = ln(1 + 2/num_buckets) > 0`.
- Committed `t5-small.json` uses 32/128 — unaffected; both refusals
  have dedicated tests (`tests/arch.rs:2132-2163`).

### WP2 prior_art slices — CLEAN; ~12 laws spot-checked vs Python

Corpus chain verified end-to-end: engine-eqsat `prior_art()` →
`emit_laws` → kit-eqsat `data/laws/prior_art.json` (135 records) →
eqsat `emit_eqsat_laws.py` → `eqsat_laws.json` (488 total).
`pytest tests/test_law_bank_corpus_parity.py
tests/test_eqsat_laws_corpus.py` — **8 passed** at eqsat HEAD
(`a4fe2f9`, 488 records — the §1 concat block `d9b7a41`/`0cbd0bf`/
`a4fe2f9` landed after the scope note's 468 and is also green).

Spot-checks against `eqsat:packages/mlopt-core/src/mlopt_core/
laws/priorart.py`:

- **`unsqueeze_squeeze`** (`builtin.rs:1877`) — `_check_unsq_sq`
  re-authored as `mod(UD, len(shape u)+1) == mod(SD, …)` with
  `attr-type int` gates; `mod` is `py_mod` (floored, Python sign —
  `guard.rs:1116`, tested `-3 % 2 == 1` at `:2666`).  Rank-0 edge
  (`len(s)=0` → modulus 1 → both sides 0 → fires) matches Python's
  `ud % 1 == sd % 1`.  ✓
- **`squeeze_unsqueeze`** (`:1918`) — `attr-cmp-dim 1 == u D`.
  `dim_at` (`shape.rs:2690`) accepts `-rank ≤ i < rank` via
  `rem_euclid` and declines out-of-range; Python's `s[d % len(s)]`
  wraps *any* int.  **Strictly narrower**: a spelled out-of-range
  dim (e.g. `dim=5` on rank-3, which torch itself rejects) fires in
  Python, declines in Rust — safe direction, noted INFO.
- **`roll_noop`/`pad_noop`/`repeat_noop`** (`:2163`/`:2196`/`:2223`)
  — the unique+sum folds: `len(unique)≤1 ∧ sum(unique)==0` ≡
  `all(v==0)`; repeat adds `len(S)==rank` (the full-cover half) +
  the all-ones fold with an empty-sequence arm.  Empty attrs read
  `unique→()`, `sum→0` matching `all(())`.  ✓
- **`permute_noop`** (`:2267`) — verified the subtle one:
  `posmap(P,P)` yields *first-occurrence positions* (`guard.rs:1407`),
  so for distinct `P` it produces `[0..n)` regardless of `P`'s
  values; `posmap(P,P)==P` therefore forces `P == range(rank)` —
  exactly `_check_permute_id`.  The `len(unique(P))==len(P)`
  distinctness conjunct is load-bearing (without it `P=[0,0,…]`
  would self-map) and present.  ✓
- **`flatten_noop`** (`:2388`) — `mod(SD,rank) == mod(if attr-is ED
  null then -1 else ED, rank)`; the bound-`null` `end_dim` arm
  reproduces Python's `ed = -1 if ed is None`, rank-0 declines via
  `py_mod(_,0) → None` like Python's `and s` gate.  ✓
- **`flip_involution`** (`:2497`) — `attr-ints D1` +
  `attr-eq-attr D1 D2` ≡ `_check_flip_inv` (int-tuple + equality;
  bool admitted on both sides).  ✓
- **Declarative block** (`fe6353d`): `std_sqrt_var`/`var_square_std`
  (+bare pair) carry the shared `dim`/`correction`/`keepdim`
  metavars verbatim with `derivation` provenance; `norm2_expand`/
  `norm2_fold` pin `ord` as `af_lit(2)`; `log_softmax_fold`/
  `expand`, `sigmoid_expand` (`Pat::constant(1)` ≡ `Const(1)`),
  `tanh_expand` (`sub(mul(2,sigmoid(mul(2,x))),1)` — verbatim) all
  match `priorart.py:1490-1748`.  `concat_single`/`concat_flat_l`/
  `concat_flat_r`/`concat_chunk2`/`concat_chunk3` (`d9b7a41`)
  verified against `priorart.py:373-430` — shared `D` metavar
  across concat+chunk dims, literal `chunks`/`index` pins.  ✓
- Honest-decline census intact: `squeeze_all_noop`/`expand_noop`
  stay Python-side (documented at `builtin.rs:1870` area +
  `priorart` pack docstring); `_CHECK_TRANSLATED` grew to cover the
  8 re-authored names; parity test pins the exemption shape.

### D-1b in flight (mlopt-carriers → kit-carriers) — **one HIGH**

- **P12-N1 (HIGH): graphopt HEAD is uninstallable — the pinned tag
  does not exist.** `graphopt@1ab7e43` (which *is* tagged `v0.1.14`
  on the remote — verified `git ls-remote`) declares
  `kit-carriers = { git = "…/kit-carriers.git", tag = "v0.1.0" }`
  (`pyproject.toml:143`), but **kit-carriers has zero tags** —
  remote heads show only `main @ e192824`.  `uv lock --check`
  fails fetching `refs/tags/v0.1.0` ("couldn't find remote ref");
  `uv sync`/CI on graphopt HEAD cannot resolve.  Compounding:
  **`uv.lock` was not regenerated** — last touched at `fdf0f76`,
  still resolves `mlopt-core`/`mlopt-orchestrator`/`mlopt-dtypes`
  at eqsat **v0.1.8** (pyproject wants v0.1.9), still lists
  `mlopt-carriers` editable at `packages/mlopt-carriers` (deleted),
  and has no `kit-carriers` entry at all.  Fix: tag kit-carriers
  (e.g. `v0.1.0` at `e192824` — its own pins are already consistent:
  eqsat v0.1.9 ✓ exists, mlopt-torch graphopt v0.1.14 ✓ exists) and
  re-run `uv lock` in graphopt.
- **P12-N2 (MEDIUM): the carriers code lost its coverage floor in
  the split.** graphopt's ci.yml coverage `--source` correctly
  dropped `mlopt_carriers` (`1ab7e43`), but kit-carriers
  `pyproject.toml` has `[tool.coverage.run] source=["kit_carriers"]`
  with **no `[tool.coverage.report] fail_under`** and `ci.sh` runs
  no coverage at all — the package that lived under graphopt's
  100% wall now has no floor anywhere (same gap class as P11-N2,
  which kit-onnx just fixed; kit-carriers needs the same
  measure-then-floor commit).
- **Rename mechanics otherwise clean.** eqsat `414ec4e` renames
  `_CARRIER_MODULES`/`_coerce_source`'s bare-name prefix/docstrings
  to `kit_carriers.*` — org-wide grep finds **zero**
  `mlopt_carriers`/`mlopt-carriers` refs in eqsat `packages/` +
  `tests/`; the "consumers still on mlopt-carriers skip missing
  modules" claim holds (builtins tolerate absent modules).  graphopt
  source-side rename verified: `mlopt-torch/src/.../adapters.py:183-196`
  lazily imports `kit_carriers.*`, `mlopt-discovery` declares the
  dep (`pyproject.toml:13`), ci.yml coverage source updated, zero
  live `mlopt_carriers` refs outside ignored `__pycache__`/`build/`
  artifacts.  kit-carriers `e192824` self-registers the three
  carrier modules on `OpTable.full()` via `register_carrier` —
  correct belt-and-suspenders for pre-v0.1.9 eqsat consumers
  (importlib dedupes the overlap with `_CARRIER_MODULES`).
- **P12-N3 (INFO): mlopt + kernelopt stay self-consistent on the
  old pin set** — both pin `mlopt-carriers` at graphopt **v0.1.13**
  subdir + eqsat v0.1.8, which still exists and still contains
  `packages/mlopt-carriers`.  But `v0.1.14` now tags post-split
  HEAD where that subdir is gone — the next deliberate bump must
  switch to `kit-carriers` + eqsat v0.1.9 together (mlopt-core
  v0.1.9's `_CARRIER_MODULES` names `kit_carriers.*`; pairing it
  with mlopt-carriers@v0.1.13 would silently lose carrier bindings).
  Same class as P11-N6, now with the tag landed.
- **P12-N4 (INFO, local-only):** graphopt's working tree has a
  stale untracked `packages/mlopt-carriers/` dir (egg-info +
  `__pycache__`, no `pyproject.toml`) that breaks local
  `uv lock`/workspace discovery ("Workspace member missing a
  pyproject.toml").  Fresh clones unaffected; delete the dir.
- **P12-N5 (INFO):** `scripts/dspec_audit.py:55-61` still points at
  `packages/mlopt-carriers/src/mlopt_carriers/*.py` — already stale
  *before* D-1b (at pinned rev `4f586f8^` the package was still
  `catopt-carriers`; verified `git show` fails).  Ungated dev
  script; predates this split.

### eqsat gpt2-scale profile — diagnosis verified; fix landed and is sound

- **The Term-hash-exponential claim is consistent with the code.**
  `handl_egraph::Term` derives structural `Hash`/`Eq` over the
  Rc-shared DAG (`egraph/src/term.rs:24-45`); the pre-fix
  `engine-eqsat/src/shape.rs` `Memo` was `HashMap<Term,
  TensorInfo>` — every `contains_key`/`insert` hashed the fully
  expanded term, O(2^sharing).  `extract.rs:46-66` already had the
  `TermKey` pointer-identity memo (commit `bfe8066`), exactly as
  the report says.
- **The fix `3443774` is correct.** `Memo` is now
  `HashMap<(u8,usize,usize,usize), (TensorInfo, Term)>`
  (`shape.rs:173`) keyed by `(tag, ptr, len-or-ptr2, ptr3)` —
  pointer-equal `Rc` allocations are the same node; pointer-distinct
  terms merely miss (memo never changes a *value*, so e-class merges
  can't stale it — terms are immutable).  The memo **value keeps
  the `Term` alive**, so key pointers can't dangle or be reused —
  the subtle correctness point, handled.  `INFER_NODE_BUDGET =
  1<<18` (`:205`) caps distinct-node inferences per walk; exhaustion
  leaves nodes unvisited → `Shape::Unknown` → the standard guard
  decline — veto, never hang (report recommendation #3, also
  landed).  Postorder driver verified terminating (memo-hit skip +
  marker re-push; pushes bounded by DAG edges).
- **Housekeeping resolved:** the report's "unpushed" egraph commits
  (`7bed824` + the `EQSAT_DEBUG` probe) are now committed and
  **pushed** — egraph `main = 2b83e74` = `origin/main`; the probe
  is no longer uncommitted WIP.  engine-eqsat `b8debe8`'s
  `EQSAT_TIMING` pair landed as `b8debe8`+`7bed824`, both pushed.
- **WIP test failure explained, not a regression:** `cargo test`
  shows `laws_corpus_files_load_and_match_the_builtin_packs`
  failing at `contracts.rs:616` — caused by the *dirty*
  `builtin.rs`/`contracts.rs` (prior_art 135→153 mm-merge block
  ahead of the corpus regen), not committed state.  Committed pin
  is 135 = kit-eqsat's committed 135-record pack.  All other test
  binaries passed.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P12-N1 | HIGH | `graphopt/pyproject.toml:143` vs `kit-carriers` remote | **Pinned tag `v0.1.0` does not exist** — kit-carriers has zero tags (main @ `e192824` only); `uv lock --check` fails on the fetch. Compounded by stale `uv.lock` (eqsat v0.1.8, phantom `mlopt-carriers` editable, no kit-carriers entry — last regenerated at `fdf0f76`). graphopt HEAD (= remote tag `v0.1.14`) cannot `uv sync`. Fix: tag kit-carriers + `uv lock`. |
| P12-N2 | MEDIUM | `kit-carriers/pyproject.toml`, `kit-carriers/ci.sh` | No coverage floor anywhere for the split-out carriers — `[tool.coverage.run]` exists but no `fail_under`, ci.sh runs no coverage. The code escaped graphopt's 100% wall. Same fix pattern as kit-onnx `6b1766c`: measure, set floor, wire into ci.sh. |
| P12-N3 | INFO | `mlopt/pyproject.toml:64`, `kernelopt/pyproject.toml` | Both pin `mlopt-carriers` @ graphopt `v0.1.13` + eqsat `v0.1.8` — self-consistent today, but `v0.1.14` tags post-split HEAD (subdir gone); next bump must move to kit-carriers + eqsat v0.1.9 atomically or carrier bindings silently vanish. |
| P12-N4 | INFO | `graphopt/packages/mlopt-carriers/` (untracked) | Stale egg-info/`__pycache__` dir with no `pyproject.toml` breaks local `uv lock` workspace discovery; fresh clones unaffected — delete locally. |
| P12-N5 | INFO | `graphopt/scripts/dspec_audit.py:55-61` | Revision-pinned paths already stale pre-D-1b (catopt-carriers era); ungated dev script. |
| P12-N6 | INFO | engine-eqsat / kit-eqsat / kit-carriers worktrees | In-flight WIP: prior_art 135→153 (mm/linear/conv merges) engine+kit sides consistent, corpus regen pending; kit-carriers `ty==` pin + untracked uv.lock. Committed state verified consistent (488=488, parity green). |
| P12-N7 | INFO | `engine-eqsat/src/shape.rs:2690` (`dim_at`) | `attr-cmp-dim` requires `-rank ≤ i < rank`; Python's `% len(s)` wraps any int — strictly narrower (declines out-of-range spelled dims Python would fire on; torch rejects those anyway). Safe direction. |

No HIGH findings beyond P12-N1.  No honesty-invariant violations:
every new refusal names its cause (`key_prefix` collision, t5
bucket bounds, guard declines via `Unknown`), the injectivity check
covers every checkpoint-key path with tying preserved, the WP2
cond re-authorings are equal-or-strictly-narrower vs the Python
hooks (`py_mod` is Python-faithful; the only asymmetries found
decline where Python fires, never the reverse), the T-101 clamp's
dead-element proof holds, and the shape-memo fix converts a hang
into the standard `Unknown` decline under a node budget.

**Verdict: PASS with one HIGH** — everything audited is mechanically
clean and the WP2/D-1b renames are faithful, but **P12-N1 must be
fixed before anyone runs `uv sync` on graphopt HEAD**: the
kit-carriers `v0.1.0` tag was never pushed and `uv.lock` is a
release behind.  P12-N2 is the same coverage-floor gap kit-onnx
just closed — worth the same one-commit fix while the split is
fresh.
