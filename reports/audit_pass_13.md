---
title: Audit pass 13 — WP2 law-port closure + identity memos + T5 flatten + D-1b installability cascade
type: report
date: 2026-11-16
---

# Audit pass 13

Scope: committed state since pass 12 — WP2 prior_art completion
(engine-eqsat `60d03ad`/`d777a84`, kit-eqsat `7b66ee0`/`04d8064`,
eqsat `cdc0771`/`0bb5940`; corpus now **524** records, prior_art
**171**), the identity-keyed shape/cost memos (engine-eqsat
`3443774`, egraph `bfe8066`; egraph `2b83e74` is the EQSAT_DEBUG
timer follow-up, not a memo change), the T5 source-onnx fixes
(`edb8a75` degenerate-spec refusals, `6bec25d` rank-1 bucket-index
flatten; `7f4d145` key_prefix injectivity re-verified in context),
and the D-1b installability cascade (kit-carriers `4dd89dd`/
`a25613f`, graphopt `1ab7e43`/`8e4516d`, eqsat `414ec4e`, mlopt
`7bf7677`, kernelopt `e3b45d5`).  The scope note's `bd64dd14` is
**not present** in any audited repo (`git cat-file -t` misses
everywhere); the actual D-1b fix commits are the ones above.

**WIP present and not disturbed:** engine-eqsat `tests/eqsat.rs`
is dirty with rustfmt-only churn; mlopt has uncommitted
`scripts/train_delivery_prior.py`, `src/mlopt/deliver_game.py`,
plus untracked `arch-t5-small.hnl`/`arch-t5-small.safetensors`;
host has unrelated `Cargo.lock`/`docs/site/...` modifications.
Committed sides verified consistent: kit-eqsat 524 = eqsat
`eqsat_laws.json` 524 = engine-eqsat builtin packs; parity suite
green at HEAD.

## Per-commit verdicts

### WP2 prior_art closure — CLEAN; the Python-only remainder is named

Independent census (not the comments): Python bank **544** records,
kit corpus **524** records (464 non-`prim:` + 60 `prim:*`), Python-only
**20**.  Focused `prior_art`: Python **189**, kit **171**, gap
**18** — exactly the documented permanent declines:

- `squeeze_all_noop`, `expand_noop` — all-dims forall / `-1`
  keep-dim reads the expr algebra cannot shape
  (`test_law_bank_corpus_parity.py:17-36`);
- the ten `REDUCE_DIM1` rules — `amax_dim1_{drop,keep}`,
  `amin_dim1_{drop,keep}`, `mean_dim1_{drop,keep}`,
  `prod_dim1_{drop,keep}`, `sum_dim1_{drop,keep}` — `_COND_DIM1`'s
  `attr0`-inside-`attr-cmp-dim` spelling (`priorart.py:346-366`);
- `mean_expand`, `mean_fold` — product-over-dims derive +
  Const-divisor tensor read;
- `layer_norm_expand_{plain,affine}`, `instance_norm_expand`,
  `group_norm_expand` — `range`-over-dims axis tuple +
  `prod`/divisibility vetoes (`builtin.rs:8330-8334` names the same
  wall).

The two non-prior_art Python-only records are also named permanent
declines: `conv_bn_fold` is a minted composite carrying transported
procedural `check`/`derive` hooks (`tensor.py:2886-2900`), and
`rope_cat_compose` needs attr-arithmetic vs shape-extent comparisons
the `cond` vocabulary cannot form (`attention.py:234-260`,
`308-325`; `builtin.rs:6858-6866` declines it by name).  No silent
unported rule found.

Parity mechanism is not a gap-hider: `test_law_bank_corpus_parity.py`
still enforces kit-record→bank coverage (`:618-629`), op-tree parity
(`:632-648`), canonical attrs with `_ATTR_DIVERGENT = frozenset()`
(`:318`,`:651-667`), record fields with `_FIELD_DIVERGENT =
frozenset()` (`:326`,`:670-685`), minted `prim:*` verbatim
(`:688-712`), and the exact check→cond translation census
(`_CHECK_TRANSLATED` = **122**: 36 attention + 77 layout + 8 prior_art
no-op re-authorings + `rope_rh_to_cat`; `:328-496`, pinned at
`:715-753`).  Ran `.venv/bin/python -m pytest
tests/test_law_bank_corpus_parity.py tests/test_eqsat_laws_corpus.py
-q` → **8 passed**.  kit-eqsat `cargo test --workspace -q` → **8
passed**; its `corpus_drift.rs` pins the per-pack counts and 524
total (`corpus_drift.rs:52-59`,`:149-159`; `lib.rs:26-40`,`:219-221`).

### Deep spot-checks of the newest ports — CLEAN

- **`rope_rh_to_cat`** — Python `_check_rope_equiv`
  (`priorart.py:207-248`) vs Rust cond (`builtin.rs:8344-8441`).
  Every operand maps: `rank x > 0` ≙ non-empty shape tuple;
  `attr-type int` on `CD/I1D/I2D/I1A/I1E/I2A/I2E` ≙ the `isinstance`
  gates; `CD >= 0` + `CD < len(shape x)` ≙ `0 <= axis < nd`;
  `I1D == CD`, `I2D == CD` ≙ the same-axis requirement;
  `mod(nth(shape x, CD), 2) == 0` ≙ `width % 2 == 0`;
  `I1A == 0`, `I1E == floordiv(width,2)`, `I2A == floordiv(width,2)`,
  `I2E >= width` ≙ `a1==0/e1==h/a2==h/e2>=width`.  Patterns match
  `_rope_rh_pat`/`_rope_cat_pat` (`priorart.py:2397-2434` vs
  `builtin.rs:8345-8395`), including the shared `CD` concat-dim
  metavar and the `cat(c,c)`/`cat(s,s)` doubled-table spelling.
  `None` shape extents decline through `Expr::Shape`/`Nth` → `None`
  → `attr-cmp` false (`guard.rs:1608-1674`,`:1420-1436`,`:980-983`).
- **`linear_out_channel` / `_rev`** — Python
  (`priorart.py:2483-2533`, `_COND_LINEAR_XW` `:331-336`) vs Rust
  (`builtin.rs:8498-8597`).  `spec_wt()` is the same
  `["transpose-out","W",null,null]` (`builtin.rs:5469-5470`);
  `mm-shape-ok` + `rank W == 2` + `shaped c` + rank-1-or-unit-channel
  `c` + `dim-eq c -1 W 0` match verbatim; `dspec CV = (dim(W,0),1)`
  matches `_DSPEC_OUT_CH`; the reverse adds the `CV` attr-cmp and
  `derivation`.  Reshape/dspec behavior is the declared `(o,1)`
  output-channel view — no silent shape guess.
- **`onehot_matmul_embed`** — Python (`priorart.py:2563-2585`) vs Rust
  (`builtin.rs:8670-8707`): `double(one_hot(i,N)) @ W →
  embedding(W,i)` with `rank W == 2` and `dim(W,-2) == N`.  The rank-2
  weight / class-count / input-channel relationship is exactly the
  oracle's; rank>1 index behavior is the backend's honest-decline
  contract (`emit.py:1103-1125`, `aten_kernels.rs:1924-1953`), not a
  law-side widening.
- **`select_to_slice` / `slice_to_select`** — Python
  (`priorart.py:965-1033`) vs Rust (`builtin.rs:2413-2420`,
  `2489-2577`, `cond_axis` `2790-2797`).  `_COND_AXIS` (nonneg int
  `D` in range) + `I` int/nonneg + `shape[u][D] > I` match;
  `dspec E = I+1` and the reverse's `E == S+1` + `derivation` are
  verbatim.  No axis-normalization drift: both sides require the
  nonneg spelling.
- **`conv_out_merge*` family** — Python `_make_conv_merge_rules`
  (`priorart.py:826-921`) vs Rust `PA_CONV_TAILS`/`pa_conv_merge_rules`
  (`builtin.rs:1794-1925`).  Three attr tails × bias arity = six
  rules; the shared `ST/PD/DL/GR` metavar names bind both convs *and*
  the merged conv to the same spelling.  `_spdg` adds
  `attr-eq GR == 1` plus the input-channel `dim-eq` — grouped convs
  correctly refuse the plain output-bank concat.  Output concat rides
  `axis x CD 1` (rank-4 NCHW channel axis); weights concat on `dim=0`,
  biases likewise rank-1 `dim=0` with `b1/b2` extent guards.  Kit JSON
  confirms all six names and the `GR` conjunct only on `_spdg`
  records.

### Identity memos — CLEAN; `Unknown` declines, never passes

- **engine-eqsat `3443774`** — `Memo` is now
  `HashMap<(u8,usize,usize,usize),(TensorInfo,Term)>`
  (`shape.rs:173`), keyed by `(tag, ptr, len-or-ptr2, ptr3)`
  (`term_key` `:178-191`).  ABA is closed by construction: the value
  stores a cloned `Term` (`memo.insert(key,(info,node))` `:274`), so
  every `Rc<str>`/`Rc<[Term]>`/`Rc<[(Rc<str>,TermAttr)]>` allocation
  the key points into stays alive for the memo's lifetime — a freed
  address cannot be reused under a live key.  Pointer-distinct terms
  merely miss and recompute; pointer-equal `Rc` allocations are the
  same immutable node, and e-class merges never mutate terms, so no
  stale *value* is possible.  `INFER_NODE_BUDGET = 1<<18` (`:205`) is
  per `infer_postorder` walk (`:245`); exhaustion `break`s with a
  debug line (`:260-269`), leaving unvisited nodes to read
  `Shape::Unknown` through `infer_with`'s fallback (`:217-223`).
  `spec_shape_with` maps `Unknown`→`Unknown` (`:2463-2468`); shape
  predicates funnel through `tshape`/`shape` and return `false` on
  non-tuples (`:2761-2799`,`:2825-2869`); `attr-cmp` evaluates `None`
  operands to `false` (`guard.rs:980-983`); `Expr::Shape`/`Dim`/
  `Bcast` decline non-tuple/unknown inputs (`guard.rs:1608-1674`).
  `Unknown` is a wildcard only inside `broadcast` (`shape.rs:503-508`)
  and the `("bcast",…)` expr arm (`guard.rs:1650-1674`) — never a
  generic guard pass.  `cargo test --test shape -q` → **21 passed**;
  `cargo test --test contracts -q` → **20 passed**.
- **egraph `bfe8066`** — the extraction cost memo uses the same
  identity-key discipline: `TermKey` (`extract.rs:46-62`),
  `CostMemo = FxHashMap<TermKey,(f64,Term)>` (`:66`), and `priced`
  stores `(cost, term.clone())` (`:102-110`), keeping the keyed `Rc`s
  alive.  The memo is per-`cost_fn` via `cost_memos`
  (`egraph.rs:278-280`, take/put `:2065-2078`) and shared across the
  `Improving` stop's in-run extraction and the final `extract_best`
  (`egraph.rs:1821-1824`, engine-eqsat `engine.rs:366-383`) — the
  intended reuse.  Residual INFO: the outer key is `(TypeId, fn
  address)`; a *same-type* closure recycled at the same address with
  different captured state could inherit a stale memo — pre-existing
  keying, not introduced by `bfe8066`, and it can only mis-price an
  extraction choice, never mint an inequivalent term.
- **egraph `2b83e74`** — EQSAT_DEBUG per-substitution resolve/check
  timers in `apply_rule` (`egraph.rs` diff); diagnostics only, no
  matcher/union semantics touched.

### T5 fixes — CLEAN

- **`edb8a75`** — `ArchSpec::validate` refuses
  `relative_attention_num_buckets < 4` (`arch.rs:1385-1399`) and
  `relative_attention_max_distance <= num_buckets/2`
  (`:1400-1418`).  Re-derived: encoder `eff_buckets = num_buckets/2`,
  `max_exact = eff/2`; `<4` ⇒ `max_exact == 0` ⇒ `is_small` false
  everywhere ⇒ the `where` dead-branch protection the `scaled >= 1`
  clamp relies on is gone.  `max_distance <= num_buckets/2` makes
  `denom = ln(max_distance/max_exact) <= 0` on the *live* branch;
  `==` is a 0/0 NaN on observed elements the clamp cannot rewrite.
  Tests pin both refusals (`tests/arch.rs:2136-2170`).
- **`6bec25d`** — `t5_position_bias` now flattens the `[L,L]` bucket
  matrix to `[L*L]`, gathers through `embedding`, and reshapes back to
  `[L,L,heads]` before `permute(2,0,1)`/`unsqueeze(0)`
  (`arch.rs:4217-4231`).  Row-major is exact: host `reshape_like` is a
  payload-preserving relabel with an element-count check
  (`aten_kernels.rs:218-236`), and `index_select` requires a rank-1
  index and gathers row-major slices (`:1924-1953`); the emitter's
  `embedding` lowers to `index_select(W,0,idx)` and honestly declines
  rank>1 indices (`emit.py:1103-1125`).  The bucket arithmetic
  produces a logical contiguous `[L,L]` int tensor, so flatten →
  gather → reshape is the same `[i,j] → (i*L+j) → [L,L,H]` row-major
  path HF's rank-N embedding denotes; no non-contiguous bucket edge
  exists in this term path.  The op census test pins the +4 reshape
  delta (`tests/arch.rs:1973-1977`).  `cargo test --test arch -q` →
  **44 passed**.
- **`7f4d145` (context)** — `validate_key_prefix_injectivity` still
  covers every checkpoint-key path: leaf `source`s and alias
  `source`s at assembly (`arch.rs:1650-1675`, called `:1990`), with
  `verify_weights`/`bind_weights` reading the same `checkpoint_key`
  (`:1712-1744`).  Identical logical sources (weight tying) still
  pass; distinct-source collisions refuse by name.

### D-1b installability cascade — CLEAN; P12-N1 resolved

- Remote tags verified live: kit-carriers `v0.1.0`=`4dd89dd`,
  `v0.1.1`=`a25613f`; kit-onnx `v0.1.0`/`v0.1.1`; eqsat `v0.1.9`=
  `414ec4e`; graphopt `v0.1.15`=`8e4516d`; gamekit `v0.1.10`,
  discoverykit `v0.1.14` exist.
- graphopt `8e4516d` (tag `v0.1.15`) pins `kit-carriers`/`kit-onnx`
  at `v0.1.1`, eqsat at `v0.1.9`, gamekit/benchkit `v0.1.10`,
  discoverykit `v0.1.14` (`pyproject.toml:131-146`); `uv.lock`
  resolves `kit-carriers` to `a25613f…` (`uv.lock:1634-1642`),
  `kit-onnx` to `2784ce8…` (`:1645-1655`), and the eqsat/gamekit/
  discoverykit entries to the same tags (`:100`,`:982-987`,
  `:1177-1179`,`:2307-2399`).  No `mlopt-carriers`/`mlopt-onnx`
  editable or stale workspace member remains; the stale
  `packages/mlopt-carriers/` dir from P12-N4 is gone.
  `uv lock --check` → **Resolved 216 packages**.
- mlopt `7bf7677` moves the dependency and the one carrier docstring
  reference to `kit-carriers`/`kit_carriers` (`pyproject.toml:12`,
  `:60-69`; `carrier_expand.py` diff) and pins graphopt `v0.1.15` +
  eqsat `v0.1.9` + kit-carriers `v0.1.1`; `uv.lock` resolves
  kit-carriers `a25613f…` (`uv.lock:291-300`).  `uv lock --check` →
  **Resolved 78 packages**.  kernelopt `e3b45d5` does the same for its
  `probes` group (`pyproject.toml:34-36`,`:49-59`;
  `uv.lock:165-167`,`:314-394`); `uv lock --check` → **Resolved 51
  packages**.  kit-carriers `a25613f` binds its own dev env's
  `mlopt-torch` as a direct git dep (`pyproject.toml:19-25`) and
  pre-registers the torch bridge in `tests/conftest.py` so subset
  runs see the backend; `uv lock --check` → **Resolved 46 packages**.
- `bd64dd14` could not be resolved in any audited repo; treat the
  named commits above as the fix set.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P13-N1 | MEDIUM | `kit-carriers/pyproject.toml:77-80`, `kit-carriers/ci.sh:6-10`, `graphopt/pyproject.toml:285-287` | **P12-N2 still open:** the split-out carriers still have no coverage floor. `[tool.coverage.run] source=["kit_carriers"]` exists but there is no `[tool.coverage.report] fail_under`, and `ci.sh` runs lint/format/ty/pytest only. Graphopt's comment now says the floor "lives there" — it does not. Same fix pattern as kit-onnx `6b1766c`: measure, set floor, wire `coverage run/report` into `ci.sh`. |
| P13-N2 | INFO | `graphopt/scripts/dspec_audit.py:55-61` (carried P12-N5) | Revision-pinned oracle paths still spell `packages/mlopt-carriers/src/mlopt_carriers/*` at `REV_CARRIERS = "4f586f8^"`; verified `git show 4f586f8^:packages/mlopt-carriers/...` fails (exit 128) while the `catopt-carriers` path resolves. Ungated dev script; predates D-1b but now also post-dates the rename. |
| P13-N3 | INFO | `kit-carriers/pyproject.toml:25` vs consumers `v0.1.15` | kit-carriers' own dev dep pins `mlopt-torch` at graphopt `v0.1.14`; graphopt/mlopt/kernelopt consumers now pin `v0.1.15`. Dev-only, consistent today (the subdir exists at both tags), but the next carrier-touching change should bump it with the consumer cascade. |
| P13-N4 | INFO | `egraph/src/egraph.rs:278-280`,`:2065-2078` | Cost-memo outer key is `(TypeId, fn address)`; a same-type cost closure recycled at the same address with different captured state could inherit a stale memo. Pre-existing keying (not `bfe8066`), and the blast radius is extraction choice only — terms remain e-class members. |
| P13-N5 | INFO | engine-eqsat `tests/eqsat.rs`; mlopt `scripts/train_delivery_prior.py`, `src/mlopt/deliver_game.py`; host `Cargo.lock`, `docs/site/explanation/platform-architecture.md` | Uncommitted WIP left untouched: engine-eqsat diff is rustfmt-only; mlopt diff is an eqsat-timeout pass-through plus an `n_enodes` DAG count (not audited as committed state); untracked T5 artifacts remain in mlopt; host has unrelated doc/lock edits. |

No HIGH findings.  P12-N1 is fixed end-to-end: every consumer that
needed `kit-carriers` now pins `v0.1.1` and locks resolve.  The WP2
port is honest — the 20 Python-only records are all named permanent
declines (18 prior_art + `conv_bn_fold` + `rope_cat_compose`), the
parity guard still compares coverage/op-trees/attrs/fields/prim
records and pins the exact 122-name check→cond census, and the deep
spot-checks found no semantic drift (only equal-or-narrower guard
posture, same as pass 12).  The identity memos are ABA-safe because
the memo values keep the keyed `Term`s alive, budget exhaustion
funnels to `Unknown`, and every guard path treats `Unknown` as a
decline.  The T5 flatten is row-major exact for the logical
contiguous bucket tensor and the new refusals close the only
non-finite live-branch path found.

**Verdict: PASS with one MEDIUM** — WP2 closure, the newest law
ports, both identity memos, the T5 fixes, and the D-1b cascade are
mechanically clean and installable (`uv lock --check` green in
graphopt/mlopt/kernelopt/kit-carriers).  **P13-N1** is the carried
coverage-floor gap: kit-carriers escaped graphopt's 100% wall in the
split and still has no floor or coverage gate — worth the same
measure-then-floor commit kit-onnx already did.
