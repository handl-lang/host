# P3 Step D — graphopt→mlopt merge: impact map & sequencing

## A. Where "mlopt" is load-bearing (complete census)

### A.1 The `mlopt` distribution name is claimed TWICE today
- `mlopt/pyproject.toml:2` — `name = "mlopt"`, real product: `src/mlopt`, `[project.scripts] mlopt = "mlopt.cli:main"` (line 40), hatch `packages = ["src/mlopt"]` (line 47).
- `graphopt/pyproject.toml:6` — `name = "mlopt"`, **empty meta-dist** (`packages = []`, line 110; "no importable `mlopt` package", line 8). Latent collision — nobody installs a repo root (all consumers pin `subdirectory`), but it must be renamed/dropped at merge.

### A.2 `mlopt-*` package names (dist = import, underscore form)

| Packages | Repo | Status |
|---|---|---|
| `mlopt-core`, `mlopt-orchestrator`, `mlopt-dtypes` (+`mlopt-native` maturin, workspace-excluded) | **eqsat** `packages/` | current substrate home |
| `mlopt-torch`, `mlopt-carriers`, `mlopt-cuda`, `mlopt-discovery`, `mlopt-onnx`, `mlopt-host-tensor`, `mlopt-handl-peer` | **graphopt** `packages/` | domain packages — Step D.1 split candidates |
| `mlopt` (product) | **mlopt** `src/mlopt` | the merge payload |
| `mlopt-kernel` (dist) / `mlopt_kernel` (import) | **kernelopt** | codegen domain — becomes `kit-kernel` per ADR, later phase |
| `handl-host-tensor` (maturin/PyO3, module `handl_host_tensor`) | **host-tensor** | vs. graphopt's `mlopt-host-tensor` Python package — confusingly similar names, different things |

**Historical collision note:** graphopt *used* to ship `mlopt-core`/`mlopt-orchestrator`; those moved to eqsat in plan 0032 phase 4. Current `graphopt/packages/` contains none of them. Whether old graphopt *tags* still contain `packages/mlopt-core` is unverifiable without git — see §E.

### A.3 uv git pins (`[tool.uv.sources]`, all HTTPS `tag =`)

| Consumer | Pins | Tag |
|---|---|---|
| `mlopt/pyproject.toml:60-69` | eqsat ×3 (core/orchestrator/dtypes), **graphopt ×5** (torch/carriers/cuda/discovery/host-tensor), gamekit, discoverykit | eqsat `v0.1.8`, **graphopt `v0.1.13`**, gamekit `v0.1.9`, discoverykit `v0.1.12` |
| `kernelopt/pyproject.toml:50-59` | eqsat ×3, **graphopt ×3** (torch/carriers/discovery), gamekit ×2, discoverykit, lib-kernel-ir (whole-repo) | same tags; lib-kernel-ir `v0.1.1` |
| `graphopt/pyproject.toml:132-146` | eqsat ×3, gamekit ×2, discoverykit; domain pkgs `{ workspace = true }` | `v0.1.8`/`v0.1.9`/`v0.1.12` |
| `gamekit/pyproject.toml:41` | eqsat mlopt-core (benchkit only) | `v0.1.8` |
| `discoverykit/pyproject.toml:37-39` | eqsat ×2, gamekit | `v0.1.8`/`v0.1.9` |

**Zero consumers pin `handl-lang/mlopt` or the graphopt root dist** — verified by org-wide grep. The product repo is a leaf.

### A.4 Rust/cargo surface
- `mlopt-kit` crate — `graphopt/packages/mlopt-kit` (workspace member; `hnl-run` + `egraph-run` bins). Its `Cargo.toml` deps float `branch = "main"` on ~15 handl repos.
- `mlopt-experiments` — `graphopt/packages/mlopt-experiments`, standalone crate with `mlopt-kit = { git = ".../handl-lang/graphopt", branch = "main" }` (Cargo.toml:29) + `[patch]` to `../mlopt-kit` (line 61) + `.handl.toml:34` git source.
- `hnl-ext-mlopt` + `handl-kit-mlopt` — **already in kit-prelude** (Step B): `kit-prelude/Cargo.toml:3` members; `kit-prelude/crates/kit-prelude/Cargo.toml:36` and `hnl-ext-mlopt/Cargo.toml:16` dep via `git = ".../kit-prelude", branch = "main"`. Artifact `libhnl_ext_mlopt.so` via `HANDL_EXT_DIR`/`~/.handl/ext/`.
- `handl-kit-mlopt` appears in ~40 Cargo.lock files org-wide (transitive via `handl-kit-prelude`).
- `mlopt/Cargo.lock` is **stale** — contains `hnl-ext-mlopt` (line 22) but the repo has no `Cargo.toml`/`crates/` (Step B leftover; delete at archive).

### A.5 Hardcoded repo-name/path couplings (the sneaky ones)
1. **`mlopt/src/mlopt/emit.py:2815-2820`** — `_hnl_run_candidates` hardcodes `sibling / "graphopt" / "target"` and `sibling / ".ci-target" / "graphopt"`. Breaks on repo/dir rename; must become repo-agnostic or repointed.
2. **`graphopt/ci.sh`** — `graphopt` name hardcoded ~8×: `.ci-target/graphopt` (line 39), `../graphopt` self-symlink logic (lines 45,50,52,56,60,71). Travels with the renamed repo → must be renamed inside it.
3. **`.ci-target/<repo>` convention** — per-repo-name shared cargo target dirs (`host`, `kit-prelude`, `dogfood`, `graphopt` seen). Rename ⇒ new `.ci-target/mlopt`, migrate/prune old.
4. **`mlopt/src/mlopt/adapters/onnx_rust.py:79-88`** — `_MLOPT_ROOT = parents[3]`, sibling lookup `_MLOPT_ROOT.parent / "source-onnx"`; `arch_rust.py` same pattern via `SOURCE_ONNX_REPO`/`SOURCE_ONNX_ARCH`/`SOURCE_ONNX_LIFT` env overrides. **Layout-sensitive**: moving `src/mlopt` → `packages/mlopt/src/mlopt` adds a path level and silently breaks sibling resolution (parents[3] would land on `packages/mlopt`, not repo root). Same for `emit.py:2806` `_REPO = parents[2]`.
5. **`kit-prelude/crates/kit-mlopt/src/lib.rs`** — spawns `python -m mlopt stage|<cmd>`; interpreter = `MLOPT_PYTHON` env → `python3` (lines 145-149, 286-295); `MLOPT_PIPELINE_CMD` escape hatch (~line 188). **The `mlopt` import name is a live cross-repo contract.**
6. **Process configs**: `mlopt/bupha.toml` (`name`+`dir`), `docs/bupha.toml:31` (watches graphopt for docs deploys), shared `~/.config/bupha/bupha.toml` (outside repos). `kernelopt/ci.sh` iterates `$WORKSPACE_ROOT/*/` — name-agnostic. No `../graphopt`/`../mlopt` path deps exist in any Cargo.toml.
7. **`~/.cache/devin/handl_deps.env`** — 174 `insteadOf` entries; **missing entries for graphopt, mlopt, eqsat, gamekit, discoverykit, kernelopt, kit-eqsat, engine-eqsat, source-onnx** (host-tensor present). Contains stale `research-mlopt`→graphopt-era mapping (key 168). Must add graphopt→mlopt (+ likely the missing repos) at rename.
8. **`graphopt/manifest/kit.toml`** — `name = "graphopt"`, `mlopt-kit` entry `repo = "handl-lang/graphopt"` (line 95), `name = "mlopt"` entry `repo = "handl-lang/graphopt"` (lines 107-108).
9. **Docs links** to `github.com/handl-lang/graphopt`: `host/docs/site/explanation/{platform-architecture.md:80,kits-and-manifests.md:125}`, `host/paper/{README.md:27,30,references.bib:56}`, `eqsat/docs/site/reference/frozen-api.md:49`, `gamekit/docs/site/reference/frozen-api.md:68`, `kernelopt/README.md:4`, graphopt `docs/site/**` self-links, pitch slides. GitHub redirects cover them; update canonical ones later.
10. **CI**: graphopt has GHA `ci.yml`+`bench.yml`+`ci.sh`; mlopt has `ci.sh`+`bupha.toml`; kernelopt `ci.sh`; eqsat/gamekit GHA `ci.yml`; discoverykit has none found.

## B. Collision analysis

- **Repo name**: `handl-lang/mlopt` occupied ⇒ graphopt cannot take `mlopt` until old mlopt is renamed (archive alone does **not** free the name — GitHub keeps it). Renaming old mlopt first leaves a window where `handl-lang/mlopt` 404s/redirects to the interim name — harmless since nothing git-installs it, but the *product-preserving* order avoids even that (see §C).
- **Dist name `mlopt`**: two pyprojects claim it; harmless today (empty meta-dist, subdir pins) but must be resolved — recommend renaming graphopt's root to `mlopt-meta` (matching `gamekit-meta`/`discoverykit-meta` precedent) in the merge commit.
- **Import name `mlopt`**: only product repo ships it; merge must keep `python -m mlopt` + `import mlopt` working at the pinned location or `handl-kit-mlopt` breaks.
- **Package names**: ADR moves *contents* to `kit-torch`/`kit-onnx`/etc. Whether dist names rename (`mlopt-torch`→`kit-torch`, `mlopt_core`→`kit_core`?) is undecided — renaming cascades through every import, import-linter contract, coverage `source=`, ty `extra-paths`, all pins, frozen `docs/public_api.md`. **Recommend keeping `mlopt-*` names in Step D** (ADR: "names are scaffolding; layers are the structure").
- **`mlopt-kit` vs `handl-kit-mlopt`**: near-identical crate names in different repos — confusing but not colliding; plan §2 says mlopt-kit "absorbs into renamed-mlopt". Naming decision needed at merge.
- **Tag/subdir safety**: tags are immutable; `tag=`+`subdirectory=` pins either resolve correctly at a tag or fail loudly — no silent wrong-content window. Old graphopt tags remain valid forever as rollback anchors.

## C. Recommended sequencing (keeps every `uv sync`/`cargo`/`python -m mlopt` green)

**D-0 — decisions + freeze (no code)**
- User decisions (§D). Freeze tags: confirm remote tags via `git ls-remote --tags` on all repos; ensure mlopt/kernelopt/graphopt pins are on latest (`v0.1.13`/`v0.1.8`/`v0.1.9`/`v0.1.12`).

**D-1 — graphopt content split (repo still named graphopt)**
Per ADR map, leaf-first, each move = new repo + tag + repin + `uv lock` + suite green:
1. `mlopt-host-tensor` → follows host-tensor/`lib-tensor` move (check ADR-0175 phase gate).
2. `mlopt-torch` → `kit-torch`; `mlopt-onnx` → `kit-onnx`; `mlopt-cuda` → cuda-sink kit; `mlopt-carriers` → kit data or kit-torch.
3. `mlopt-discovery` → discoverykit-side or kit data (**note**: mlopt product deps on it for `hnl_import`/`hnl_surface` — its destination determines the product's pin).
4. `mlopt-kit`/`mlopt-experiments`/`mlopt-handl-peer` → absorb into renamed-mlopt or kit homes.
- Repin per move: `mlopt/pyproject.toml` (5 pins), `kernelopt/pyproject.toml` (3+probes), graphopt workspace itself shrinks.
- Gate: all consumers' `uv sync` green on new tags; old graphopt tags still serve old subdirs (rollback path intact).

**D-2 — free the name, merge, rename (the atomic-ish step)**
1. Rename `handl-lang/mlopt` → `handl-lang/mlopt-product` on GitHub (frees `mlopt`; redirect mlopt→mlopt-product live).
2. In graphopt: merge `mlopt-product` content — **choose layout carefully**: top-level `src/mlopt` + root pyproject converted to the product (name `mlopt`, hatch) preserves `parents[3]`/`parents[2]` depth math and `python -m mlopt`; `packages/mlopt` workspace member is cleaner organizationally but **breaks `_MLOPT_ROOT`/`_REPO` depth assumptions and adds a path level** — must fix `emit.py:2806`, `onnx_rust.py:80`, `arch_rust.py` sibling logic, plus `graphopt/ci.sh`-style path assumptions in the same commit.
3. Rename graphopt root dist `mlopt`→`mlopt-meta`; add/update `mlopt-product`'s deps to point at the *new* kit-* repos (post-D-1), `uv lock`.
4. Verify in merged repo: `uv sync`, `pytest`, `python -m mlopt stage lift` smoke, `hnl mlopt` parity via kit-prelude.
5. Rename `handl-lang/graphopt` → `handl-lang/mlopt` (name now free). GitHub redirects keep `graphopt.git` URLs working.
6. Immediately after: repin `mlopt/pyproject.toml`+`kernelopt` leftovers, `mlopt-experiments` Cargo dep/patch/`.handl.toml`, `manifest/kit.toml`, `docs/bupha.toml`, `mlopt/bupha.toml`→new name, `emit.py` `.ci-target`/sibling candidates, `graphopt/ci.sh`→`mlopt/ci.sh` self-name strings, `handl_deps.env` (add `mlopt`+missing repos, drop `research-mlopt`), local dir rename `~/GitHub/handl-lang/graphopt`→`mlopt` (check `git worktree list`, unpushed commits first — repo deletion checklist).
7. Archive `mlopt-product` (read-only) after a soak; delete stale `mlopt/Cargo.lock`.

**Why merge-before-rename (D-2.2 before D-2.5)**: the name `mlopt` is only ever occupied by a repo that *contains* the product; the alternative (rename first, merge after) leaves `handl-lang/mlopt` = domain-repo-without-product — legal per ADR but a worse story for `git+.../mlopt` installs and `MLOPT_PYTHON` docs. ADR's own open question #5 flags this ordering; recommend sub-steps 3→2 (merge content, then rename).

**Rollback**: tags immutable — every step's prior tag is a restore point; GitHub renames are reversible; `uv.lock`/Cargo.lock changes are git-revertible; keep `mlopt-product` un-archived until soak passes. Do NOT delete `mlopt-product` — archive only.

## D. Open questions needing user decision

1. **Final repo name** — ADR ratifies `mlopt` for merged repo; confirm (alternatives: `mlopt-product`, keep `graphopt`).
2. **Python package names** — do `mlopt-torch`/`mlopt_core`/etc. rename to `kit-*` in Step D, or stay `mlopt-*`? (Recommend stay; separate later decision.)
3. **Merge layout** — `src/mlopt` at root vs `packages/mlopt` workspace member (affects `_MLOPT_ROOT`/`_REPO` depth math — see A.5.4).
4. **graphopt meta-dist** — rename to `mlopt-meta`, `graphopt-meta`, or drop root project entirely?
5. **Old mlopt repo fate** — rename to `mlopt-product` + archive (recommended), or delete after merge (check unpushed commits/stash/worktrees first per global rules).
6. **`mlopt-discovery` destination** — discoverykit-side vs kit data vs stays-with-product (it carries `hnl_import`, the `.hnl` codec the product needs).
7. **`hnl mlopt` command name** — survives as kit-registered command? ADR-0175 says "users never type mlopt." but adapter names (`whisper:`, `.hnl` refs) keep mlopt semantics — confirm command surface end-state.
8. **`mlopt-kit` crate naming** at absorption (`mlopt-kit` vs `handl-kit-mlopt` vs `kit-mlopt`).
9. **PyPI** — org rule forbids publishing except `tamias`; confirm `mlopt` dist never goes to PyPI (name may be taken there anyway — irrelevant for git deps).
10. **kernelopt visibility** — private repo; new kit-* repos' visibility must not break its pins.

## E. Actions I could not perform (parent must handle)

- **Write the report file** — DONE by parent (this file).
- **`git ls-remote --tags`** on graphopt/mlopt/eqsat/gamekit/discoverykit/kernelopt/lib-kernel-ir — verify actual remote tags match pins (`v0.1.13` etc.) and inspect whether old graphopt tags still contain `packages/mlopt-core`/`mlopt-orchestrator` (latent duplicate-dist risk for anyone pinning old tags).
- **`git -C <repo> status`/`log --branches --not --remotes`/`stash list`/`worktree list`** on mlopt and graphopt before any rename/archive (global repo-deletion checklist).
- **`myco status --prompt`** per AGENTS agent-awareness rules before proposing next steps.
