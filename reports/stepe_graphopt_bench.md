# Step E tail — graphopt bench drivers + codec sites (dry-run)

Date: 2026-11-16. Worktree: `/home/luis/tmp/graphopt-stepe`,
branch `stepe/graphopt-bench-codec`, commit `2159bc0` on top of
`e4c639b` (origin/main). Nothing pushed. The real checkout
`/home/luis/GitHub/handl-lang/graphopt` was clean and untouched;
the kit-torch Phase-3 agent's mass-rename lives only in
`/home/luis/tmp/graphopt-d1e`.

## Verdict: GO for the codec migration; the repin is dry-run scaffolding

`uv lock` + `uv sync --extra dev` resolve cleanly and the focused
tests pass — **but only with the mlopt-torch-era holdout pins +
`[tool.uv] override-dependencies`** documented below. The repin
report's "full set" (kit-carriers v0.1.5 / kit-cuda v0.1.5 /
discoverykit v0.1.23) is **not landable on graphopt main today**:
those tags bind `kit_torch`, which does not exist in this repo
until P3 Step D-1e removes `packages/mlopt-torch`. Verified
empirically (see "kit-torch collision" below).

## Pins — final resolution

| dep | was | now | note |
|---|---|---|---|
| mlopt-core / mlopt-orchestrator / mlopt-dtypes | v0.1.10 | **v0.1.12** (`d67d1fc8`) | carries `term` + `term_json` |
| gamekit / benchkit | v0.1.12 | **v0.1.14** (`00263df9`) | |
| kit-onnx | v0.1.2 | **v0.1.3** (`084897d4`) | |
| kit-host-tensor | v0.1.3 | **v0.1.4** (`1f5912a3`) | |
| kit-carriers | v0.1.3 | **v0.1.3 (unchanged)** | v0.1.4+ deps `kit-torch`; see below |
| kit-cuda | v0.1.3 | **v0.1.3 (unchanged)** | same |
| discoverykit | v0.1.20 | **v0.1.20 (unchanged)** | v0.1.21+ lazy-binds `kit_torch` |
| kit-discovery | v0.1.2 | **v0.1.2 (unchanged)** | newest tag; no v0.1.12-carrying tag exists |

### kit-torch collision (new finding vs repin_cascade_eqsat_v0112.md)

The earlier report's graphopt pin list (kit-carriers → v0.1.5,
kit-cuda → v0.1.5, discoverykit → v0.1.23, plus a `kit-torch` source)
was never lock-tested past the kit-discovery conflict. Applying it:

- `kit-carriers@v0.1.5` / `kit-cuda@v0.1.5` declare a runtime dep on
  `kit-torch` (handl-lang/kit-torch repo); a root `[tool.uv.sources]`
  entry alone did **not** reach the transitive requirement — only
  `override-dependencies` did (`kit-torch was not found in the
  package registry` otherwise).
- With kit-torch v0.1.2 installed beside the `mlopt-torch` workspace
  member, `tests/test_real_model_mint.py` fails: the eval path
  (`discoverykit.proposal_core._eval_backend` →
  `kit_torch.meta_eval`) and `mlopt_torch.torch_bridge._IR_TO_TORCH`
  become **two separate torch stacks** — `synop*` denotation seating
  lands in one registry while the oracle reads the other
  (`ModuleNotFoundError: No module named 'kit_torch'` once kit-torch
  was removed again; `KeyError`/`rhs-err` with it present).
- `discoverykit@v0.1.21+` has the same split: `proposal_core.py:267`
  lazily imports `kit_torch.meta_eval`; v0.1.20 is the last
  `mlopt_torch` tag. `kit-discovery@v0.1.2` itself is clean
  (zero `kit_torch` imports — verified `git grep`).

So the coherent pre-D-1e set is exactly what graphopt already pinned
for the torch-adjacent deps (kit-carriers/kit-cuda v0.1.3,
discoverykit v0.1.20) — only their *sources* are stale (eqsat
v0.1.10, gamekit v0.1.12), which the override block fixes.

### `[tool.uv] override-dependencies` (dry-run scaffolding)

```toml
override-dependencies = [
    "mlopt-core @ git+…/eqsat.git@v0.1.12#subdirectory=packages/mlopt-core",
    "mlopt-orchestrator @ git+…/eqsat.git@v0.1.12#subdirectory=packages/mlopt-orchestrator",
    "mlopt-dtypes @ git+…/eqsat.git@v0.1.12#subdirectory=packages/mlopt-dtypes",
    "gamekit @ git+…/gamekit.git@v0.1.14#subdirectory=packages/gamekit",
]
```

Same mechanism the kit-discovery stepe worktree
(`/home/luis/tmp/kit-discovery-stepe`) uses. Needed until **every**
consumed tag carries eqsat-v0.1.12 sources: kit-discovery repin+tag
AND either repinned mlopt_torch-era tags for
kit-carriers/kit-cuda/discoverykit, or D-1e removal of
`packages/mlopt-torch` so the kit-torch-era tags can be adopted.
Pyproject comment says the same.

## Codec migration — what actually moved

The task brief named `screen/discovery_scale.py`,
`screen/exposed_discovery.py`, `probes/term_ir_spike.py` as the
codec-mixed files; on inspection the real Term-JSON codec sites per
`stepe_tail_plan.md` §4 are the two audit drivers (the screen files
and spike use only the **lemma** codec `term_to_data`/`term_from_data`,
which stays in `ir.py` by design — verified, no changes needed).

| file | change |
|---|---|
| `bench/drivers/audit/onnx_lifter_differential.py` | `kit_discovery.hnl_codec.term_to_json/term_from_json` → `mlopt_core.term_json`; `mlopt_core.ir` types → `mlopt_core.term`. Canonical emit now (`attrs` field, `source:"Native"`, `Arr` leaves, dag `dim` indices) — the `_TermView` canonicalizer already merges both attrs channels and ignores `source`; docstring updated. Decode accepts both leaf spellings permanently. |
| `bench/drivers/audit/eqsat_differential.py` | term-type imports → `mlopt_core.term` (incl. two lazy in-function imports). `_kterm`/`_pyterm` **kept hand-rolled** and documented in-place as the deliberate engine-wire exception: they must round-trip the deployed `eqsat-run` wire — `const-str` leaves, `__var__:`/`__var__:param:` untyped-leaf fallbacks (the `Var(name,"f64")` shorthand has no `TensorType`, which `term_json`'s typed-leaf emit requires), and `validate=False` decode. `term_json` would decode `__var__:` as an opaque `Const` string — porting would silently lose leaves. Re-evaluate at plan Step 5 (release-binary `leaf="arr"` flip). `eqsat_shape_guards.py` rides the same codec unchanged. |
| `bench/drivers/probes/term_ir_spike.py` | **No change** — its `enc`/`dec` *is* the legacy-dialect (`Foreign` source + `lit.__mlopt_attrs__` + `__mlopt_leaf__` Arr) measurement the spike exists to record; plan says "historical; do not extend". |
| `bench/drivers/screen/{discovery_scale,exposed_discovery}.py` | **No change** — lemma codec only (`term_to_data`/`term_from_data` stay in `ir.py`); imports verified clean under eqsat v0.1.12. |
| `bench/drivers/audit/eqsat_shape_guards.py` | **No change** — class-b, rides the `ir.py` shim. |

The ~45 class-b bench files need nothing — `ir.py` is a re-export
shim over `mlopt_core.term` in eqsat v0.1.12 (`ir.Op is term.Op`
verified). Sample imports verified: all six files above import
cleanly in the synced venv.

## tests/test_public_api.py

`test_ported_public_api.py` does not exist in graphopt (it lives in
eqsat). `test_public_api.py`'s `_PUBLIC_SURFACE` is a **subset** pin
(frozen names must resolve + sit in `__all__`), so the `ir.py` entry
needed no change — but I added `mlopt_core.term` and
`mlopt_core.term_json` entries since graphopt code now binds those
modules. 257 surface tests pass.

## Verification (all in the worktree venv, eqsat d67d1fc8)

- `uv lock` — clean, 216 packages; eqsat `d67d1fc8` everywhere.
- `uv sync --extra dev` — clean.
- `pytest tests/test_public_api.py` — **257 passed, 1 skipped**.
- `pytest tests/test_real_model_mint.py tests/test_public_api.py` —
  **271 passed, 1 skipped** (this file caught the kit-torch
  split-brain — now green on the mlopt-torch-era pins).
- `pytest tests/test_morphisms.py tests/test_matmul_factor_laws.py
  tests/test_criteria.py tests/test_substrate_domain_edges.py` —
  **77 passed, 2 skipped**.
- Codec smoke: `term_to_json`/`term_from_json` round-trips tree +
  `dag.program` + `dag="tree"` + `dag=False` refusal;
  `eqsat_differential._kterm`/`_pyterm` round-trip incl. the
  `__var__:` and `__mlopt_leaf__:` leaf spellings.
- `ruff check` + `ruff format --check` on touched files — clean.
- `ty check` on touched files — 23 diagnostics, **byte-identical to
  main's baseline** on the same files (pre-existing noise, no new
  errors).
- `lint-imports` — 2 contracts kept, 0 broken.
- All six bench files `py_compile` + import cleanly.

## Go / no-go

- **Codec migration: GO.** The onnx_lifter_differential swap is
  canonical-emit and decode-compatible both directions; the
  eqsat_differential exception is documented, not ported.
- **Repin merge: NO-GO as committed.** The branch is a working
  dry-run, not a merge candidate: it carries `[tool.uv]
  override-dependencies` scaffolding and deliberately holds
  kit-carriers/kit-cuda/discoverykit at mlopt-torch-era tags. The
  real graphopt repin is gated on (a) kit-discovery's eqsat repin +
  new tag (its stepe worktree still has the shim **uncommitted**),
  and (b) either D-1e removing `packages/mlopt-torch` (unlocking
  kit-carriers v0.1.5 / kit-cuda v0.1.5 / discoverykit v0.1.21+)
  or fresh mlopt_torch-era tags with v0.1.12 sources for the three
  holdouts. Until then the override block is load-bearing, not
  scaffolding-to-delete.
- Ordering note for the parent: graphopt's repin now depends on
  **D-1e landing first** (or holdout retags) — it is no longer just
  "kit-discovery tag → bump". The mlopt/kernelopt repins have the
  same mlopt-torch-era constraint if they still ship/consume
  `mlopt-torch`.

## Files changed (commit `2159bc0`, branch `stepe/graphopt-bench-codec`)

- `pyproject.toml` — pin bumps + override block + comments
- `uv.lock` — regenerated
- `bench/drivers/audit/onnx_lifter_differential.py` — term_json swap
- `bench/drivers/audit/eqsat_differential.py` — term imports +
  engine-wire exception comment
- `tests/test_public_api.py` — `mlopt_core.term`/`term_json` surface
  entries
