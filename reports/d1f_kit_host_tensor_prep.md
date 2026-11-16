# P3 Step D-1f prep — `mlopt-host-tensor` → `kit-host-tensor` split

Read-only recon for the sixth graphopt package dissolution (D-1 =
kit-onnx, D-1b = kit-carriers, D-1c = kit-cuda, D-1d = kit-discovery,
D-1e = kit-torch — in flight). Pattern source: the kit repos'
AGENTS.md + pyprojects (all preserved history via `git subtree split`).

**Headline: this is the smallest split yet.** ~1.9k LOC of pure Python,
one real consumer (`mlopt` product), zero kit-* deps, and a hard
*runtime* dependency on the sibling `handl-lang/host-tensor` repo that
is **not** a pip dep — it's a PyO3 extension resolved by env var /
installed wheel / sibling-checkout path. See §6 for the key question.

## 1. Package census — `graphopt/packages/mlopt-host-tensor`

**Source**: `src/mlopt_host_tensor/` — 9 modules, **~1,925 Python lines**
plus one generated data file:

| module | LOC | role |
|---|---|---|
| `bindings.py` | 814 | `HOST_BINDINGS` — IR-op → host `tensor.*` positional-arg table (138 ops) |
| `wire.py` | 262 | the `{"__tensor__": …}` f64 wire codec + `__nonfinite__`/`__tensor_bin__` (ADR-0179) tags |
| `sink.py` | 205 | `HostTensorSink` — `mlopt_core.ports.Sink` impl |
| `surface.py` | 173 | `host_surface.json` reader + host-tensor-checkout surface collector (regen half of `scripts/dump_host_surface.py`) |
| `executor.py` | 146 | `HostTensorExecutor` — bottom-up term interpreter over `handl_host_tensor.call` |
| `_binding.py` | 141 | `handl_host_tensor` PyO3 ext loader (env → import → sibling `.so`) |
| `report.py` | 120 | `HostTensorVerifyReport` + NumPy equivalence gate |
| `errors.py` | ~40 | `HostTensorError` / `UnsupportedHostOpError` |
| `__init__.py` | 22 | re-exports `HostTensorSink` |
| `host_surface.json` | 2,363 lines (generated, package-data) | the 239-op `tensor.*` surface artifact, produced by `scripts/dump_host_surface.py` from the host-tensor checkout (`src/index.hnl` + `aten_port_gen --emit-spec`) |

**Declared deps** (`packages/mlopt-host-tensor/pyproject.toml:10-13`):
`mlopt-core`, `numpy>=1.24`. **Nothing else** — no torch, no
orchestrator, no kit-*. The `handl_host_tensor` extension is *not* a
declared dep; `_binding.py` resolves it lazily (import of the package
never requires the Rust build — only *using* the sink does).

**Tests**: no in-package tests dir — suite is `graphopt/tests/`.
**6 test files** reference `mlopt_host_tensor`:

| file | dep closure | verdict |
|---|---|---|
| `test_host_tensor_sink.py` (452 ln) | mlopt_core + numpy + ext (skips w/o `.so`) | **carve** |
| `test_host_tensor_wire.py` | numpy + `mlopt_host_tensor.wire` only | **carve** |
| `test_host_surface.py` | `mlopt_host_tensor.surface` + env-gated host-tensor-checkout freshness check | **carve** (env-skip machinery travels) |
| `test_surface_table_drift.py` | lazy `kit_discovery.*` + `mlopt_torch.*` imports | **stays** (cross-stack) |
| `test_onnx_codec_drift.py` | `onnx` package + `tests.test_surface_table_drift` helper | **stays** (onnx dep + cross-file helper) |
| `test_hnl_import.py` | `kit_discovery.hnl_import` + `pytest.importorskip("mlopt_host_tensor")` | **stays** (kit-discovery-bound; its drift test will need the rename) |

`scripts/dump_host_surface.py` imports `mlopt_host_tensor.surface` — it
is the artifact regenerator and **travels with the package** (it needs
the sibling `../host-tensor` checkout, which the new kit repo will also
have under `handl-lang/`).

## 2. Dep analysis — who needs whom

**kit-host-tensor's deps are all already-external**: eqsat
(`mlopt-core` @ v0.1.9 tag) + PyPI numpy + the undeclared
`handl_host_tensor` extension (env/sibling resolution, not pip).
**Zero dependency on any kit-* repo** ⇒ kit-host-tensor can be created
and tagged **first**, fully independent of D-1e. No cycle in either
direction.

**Consumers of `mlopt-host-tensor`**:

| Consumer | Kind | Current pin / reference |
|---|---|---|
| graphopt root | workspace member (`packages/*` glob) + declared dep + import-linter `root_packages` + `forbidden` source_modules + `ty` extra-paths | `{ workspace = true }` (pyproject:20,149,184,233,276) |
| **mlopt (product)** | **declared dep** + `[tool.uv.sources]` + real code refs | `graphopt@v0.1.16` subdir pin (pyproject:23,69); `src/mlopt/emit.py:1961` `find_spec("mlopt_host_tensor")` (lazy, import-light artifact read); `tests/test_emit_surface.py:36` real import; `cli.py:709` comment only (host-tensor sink has no `--backend` wiring — `choices=["torch"]`) |
| kit-discovery | import-linter `forbidden_modules` entry only (pyproject:149) — **no dep, no imports** (hnl_import.py mentions are docstrings) | config-only |
| kit-carriers | import-linter forbidden list + comments (pyproject:105,122; AGENTS ×2) | config-only |
| kit-cuda | import-linter forbidden list (pyproject:129; AGENTS ×2) | config-only |
| discoverykit | import-linter forbidden list + comment (pyproject:53,82) | config-only |
| gamekit / kernelopt / eqsat / kit-onnx | **zero references** | — |
| host-tensor repo | doc mentions only (`adr-0174` report, `emit_tensor_language.py` comment referencing `mlopt_host_tensor.surface`) | — |

Coverage note: graphopt's `[tool.coverage.run] source = ["mlopt_torch"]`
(pyproject:297) — `mlopt_host_tensor` is **not** a coverage target
today, so removing it does not touch the `fail_under=100` floor
(unlike D-1e, where the floor's subject is leaving).

## 3. Import-rename surface (`mlopt_host_tensor`/`mlopt-host-tensor`)

Org-wide real-import sites are tiny:

- **graphopt**: ~25 self-imports inside the package + the 6 test files
  above + `scripts/dump_host_surface.py` + `scripts/complexity_baseline.json`
  (**67 path-keyed entries** — regenerate via `radon_ratchet.py
  --update`) + `pyproject.toml` (5) + `uv.lock` (5) + docs
  (`host_tensor_sink_phase{0,1}.md`, `host_surface_artifact.md`,
  `arch_review_0174.md`). **No bench/ references.**
- **mlopt**: `emit.py:1961` `find_spec` string, `test_emit_surface.py`
  import, `pyproject.toml` dep+source, `uv.lock`, AGENTS.md.
- **kit-discovery / kit-carriers / kit-cuda / discoverykit**:
  pyproject import-linter `forbidden_modules` entries + comments only
  (~10 matches total, all config/docs — same "low urgency, no
  functional break" class as eqsat's `mlopt_torch` entry in D-1e).
- Lazy binds: `emit.py`'s `find_spec` is the only dynamic one;
  `test_hnl_import.py:979` `importorskip` stays in graphopt but must be
  re-spelled post-rename or it silently stops guarding.

## 4. Ordering — does D-1f wait for D-1e?

**No dependency either way.** kit-host-tensor's closure
(eqsat mlopt-core + numpy + undeclared ext) contains no kit-* repo, and
nothing in the kit layer depends on it (all edges are forbidden-list
config). It can land **before, during, or after** D-1e. The only
interaction is graphopt's own shrink: both splits delete
`packages/*` members and edit the same `pyproject.toml` sections
(sources, import-linter, ty extra-paths, workspace) — sequence the two
graphopt-side commits to avoid lockfile/pyproject merge pain, but the
kit-repo creations are independent.

One subtlety: `_binding.py:42` computes `_REPO_ROOT` as
`parents[4]` of `packages/mlopt-host-tensor/src/mlopt_host_tensor/_binding.py`.
Post-split the file sits at `src/kit_host_tensor/_binding.py` (repo
root = `parents[2]`) — **the sibling-checkout convention
(`../host-tensor`) must be re-anchored**; same for `surface.py`'s
`HOST_TENSOR_DIR` default. Both keep working under `handl-lang/` since
kit-host-tensor and host-tensor remain siblings.

## 5. Execution checklist (mirroring d1e §6)

**Phase 0 — freeze/verify**
- [ ] `git ls-remote --tags` graphopt (latest tag contains
      `packages/mlopt-host-tensor`), mlopt, kit-discovery/carriers/cuda,
      discoverykit.
- [ ] `myco status --prompt`; capture the work item.
- [ ] Confirm `handl-lang/kit-host-tensor` name is free on GitHub.

**Phase 1 — kit-host-tensor repo**
- [ ] Create `handl-lang/kit-host-tensor`; `git subtree split` of
      `graphopt:packages/mlopt-host-tensor`.
- [ ] Rename: `packages/mlopt-host-tensor` → repo-root `src/kit_host_tensor/`;
      `mlopt_host_tensor`→`kit_host_tensor`, `mlopt-host-tensor`→
      `kit-host-tensor` across `.py`, `pyproject.toml` (name,
      package-data key), docstrings. Fix `_binding.py` `_REPO_ROOT`
      (`parents[4]`→`parents[2]`) and `surface.py` host-tensor-checkout
      resolution; keep `MLOPT_HOST_TENSOR_*` env names or rename to
      `KIT_HOST_TENSOR_*` (decide — renaming breaks existing dev envs;
      precedent is to rename with the package).
- [ ] Carve the 3 package-bound tests (§1) into `kit-host-tensor/tests/`;
      `test_host_tensor_sink.py`'s skip machinery travels verbatim.
      Leave the 3 cross-stack files in graphopt.
- [ ] New `pyproject.toml` modeled on kit-cuda's: deps `mlopt-core`
      (eqsat subdir pin) + `numpy`; `[tool.uv.sources]` eqsat pin only;
      dev group pytest/ruff/coverage/ty/import-linter; coverage floor
      measured on the carved suite (expect low — most coverage came via
      graphopt's suite; ratchet convention: set at measurement);
      import-linter `forbidden` = kit_torch/mlopt_torch/kit_onnx/
      kit_cuda/kit_carriers/kit_discovery/kit_discovery-side names/
      gamekit/benchkit/discoverykit/mlopt — declared closure is
      stdlib+numpy+mlopt-core.
- [ ] Move/copy `scripts/dump_host_surface.py` (it regenerates the
      package's own artifact — belongs in the kit repo's scripts/ or
      tools/, with `HOST_TENSOR_DIR` default re-anchored).
- [ ] `AGENTS.md` (mirror kit-cuda structure: "peer backend — depends
      on eqsat mlopt-core only; the `handl_host_tensor` PyO3 ext is an
      undeclared runtime resolved by env/sibling path, NOT a pip dep"),
      `README.md`, `ci.sh` per kit convention.
- [ ] Gates: `uv sync --group dev`, `pytest tests/ -q`, `ruff check
      src`, `ruff format --check src`, `ty check`, `lint-imports`,
      coverage floor.
- [ ] Tag `v0.1.0`; push.

**Phase 2 — consumer cascade** (tiny — two real consumers)
- [ ] **mlopt**: dep `mlopt-host-tensor`→`kit-host-tensor` + source →
      `handl-lang/kit-host-tensor@v0.1.0`; rename `emit.py:1961`
      `find_spec("kit_host_tensor")`, `test_emit_surface.py` import +
      docstrings, pyproject/AGENTS/uv.lock; `uv lock`; commit.
- [ ] **kit-discovery / kit-carriers / kit-cuda / discoverykit**:
      rename the `mlopt_host_tensor` forbidden-list entries →
      `kit_host_tensor` (keep both during transition if desired);
      comments/AGENTS sweep. No dep changes, no retag strictly
      required — but each repo's next tag should carry the rename.
- [ ] **eqsat**: nothing needed (zero references).

**Phase 3 — graphopt shrink**
- [ ] Delete `packages/mlopt-host-tensor/`; `git rm` the 3 carved test
      files.
- [ ] `pyproject.toml`: drop `mlopt-host-tensor` dep + workspace
      (auto via `packages/*`), add `kit-host-tensor` git dep @ v0.1.0;
      update `[tool.uv.sources]`, import-linter `root_packages` +
      `forbidden` source_modules, `ty` extra-paths; `uv lock`.
- [ ] Rename sweep: `scripts/complexity_baseline.json` (regenerate via
      `radon_ratchet.py --update` after packages/ shrinks), remaining
      `tests/` spellings (`test_hnl_import.py:979-980`,
      `test_surface_table_drift.py`, `test_onnx_codec_drift.py`),
      `docs/` (4 reports), AGENTS.md, README.
- [ ] **Coverage-floor decision**: `source = ["mlopt_torch"]` — if
      D-1e already landed, mlopt_torch is gone and the floor needs a
      new subject or removal; kit-host-tensor is *not* currently a
      coverage target so its departure doesn't lower the floor, but
      post-D-1e graphopt may have **no** in-repo coverage source left —
      settle once, covering both splits.
- [ ] Full `scripts/gates.sh` + `bash ci.sh` (Rust arm unaffected).
- [ ] Tag graphopt next version.

**Phase 4 — docs/board**
- [ ] Update impact-map + graphopt AGENTS.md repo-split paragraph
      (add D-1f).
- [ ] `myco handoff`.

## 6. Key question — `mlopt-host-tensor` vs `handl-lang/host-tensor`

**They are complementary halves of one seam, not duplicates — and the
split should NOT merge into host-tensor.**

- `handl-lang/host-tensor` = the **runtime**: Rust crate
  (`handl-host-tensor`) implementing the 239-op `tensor.*` surface +
  `.hnl` language fragment + the `handl_host_tensor` PyO3 extension
  (`maturin` build → `handl_host_tensor.so` exposing `ops()` /
  `call(op, args_json)`). Its pyproject produces *only* the extension
  module; it has no Python package, no mlopt-core dep, no notion of
  `Sink`/`IR`/`Term`.
- `mlopt-host-tensor` = the **mlopt-side adapter**: pure Python
  `mlopt_core.ports.Sink` implementation that lowers IR terms to
  positional JSON args (`HOST_BINDINGS`) and interprets them through
  the extension. It depends on `mlopt-core` (eqsat) — a dependency the
  runtime repo must never take (host-tensor is *below* mlopt in the
  stack; making it depend on eqsat would invert the layering and
  poison every `.hnl` consumer).

Merging the Python package into the host-tensor repo would either (a)
put an eqsat dep inside the runtime repo, or (b) create a two-package
repo (maturin cdylib + setuptools Python) — both break the established
one-package-per-kit-repo convention. **Recommendation: new repo
`handl-lang/kit-host-tensor`, package `kit_host_tensor`, dist
`kit-host-tensor`** — exactly the D-1 naming pattern, sitting beside
(not inside) `handl-lang/host-tensor`. The undeclared-extension
resolution (`_binding.py`) is already repo-agnostic; only the
sibling-path anchor needs the `parents[N]` fix noted in §5.

## Risks / watch-items

- **`_REPO_ROOT`/`HOST_TENSOR_DIR` anchoring** — the only non-mechanical
  code change; verify `../host-tensor` still resolves from the new
  repo-root layout before tagging.
- **`host_surface.json` provenance** — it is generated *from* the
  host-tensor checkout; `dump_host_surface.py` + `test_host_surface.py`'s
  freshness check assume the sibling layout. Keep them together in the
  kit repo.
- **`emit.py`'s `find_spec` contract** — mlopt reads `host_surface.json`
  *without importing the package*; the package-data key rename
  (`mlopt_host_tensor`→`kit_host_tensor`) must land with the artifact
  still at the package root or `test_emit_surface.py` fails.
- **Env-var names** — `MLOPT_HOST_TENSOR_LIB/_REPO/_TARGET` are
  dev-facing; renaming to `KIT_HOST_TENSOR_*` is consistent but breaks
  anyone's env — document in AGENTS.md either way.
- **kit-discovery's `hnl_import` drift test** stays in graphopt and
  reads `HOST_BINDINGS` — post-rename it resolves through the git dep
  fine, but the *spelling* must be updated in the same graphopt commit.
