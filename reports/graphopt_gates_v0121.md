# graphopt post-migration gate run — v0.1.21 (+ af46e8f)

Date: 2026-11-16. Repo: `/home/luis/GitHub/handl-lang/graphopt` @ `af46e8f`
(`v0.1.21` + `chore(mlopt-experiments): refresh Cargo.lock transitive deps`,
landed mid-run by another agent). Worktree was clean at start; no
`git worktree add` needed. `packages/mlopt-torch` still present — the
kit-torch split (D-2?) is in flight in `/home/luis/tmp/kit-torch-work`
(separate worktree, untouched here).

Python env: `.venv` (uv-managed, Python 3.13.9). Installed pins match
`pyproject.toml [tool.uv.sources]`: mlopt-core/-orchestrator/-dtypes →
eqsat `v0.1.10`, gamekit/benchkit `v0.1.12`, discoverykit `v0.1.20`,
kit-carriers/-cuda `v0.1.3`, kit-discovery/-onnx `v0.1.2`,
kit-host-tensor `v0.1.3`. `mlopt-torch` still a workspace member.

## Static gates (scripts/gates.sh order, run individually because the
## script hard-stops at the first red gate — vulture)

| Gate | Command | Result | Verdict |
|---|---|---|---|
| ruff check | `ruff check packages scripts examples` | PASS (0 errors) | green |
| ruff format | `ruff format --check packages scripts examples` | PASS (53 files) | green |
| ty check | `ty check` | PASS (0 errors) | green |
| vulture | `vulture` (paths: packages, tests; min_confidence 80) | **FAIL — exit 3** | **pre-existing** |
| import-linter | `lint-imports` | PASS (2 contracts kept, 60 files) | green |
| bandit | `bandit -c .bandit.yaml -r packages` | PASS (0 issues) | green |
| semgrep | `semgrep --config .semgrep.yml packages` | PASS (0 findings, 3 rules) | green |
| radon ratchet | `python scripts/radon_ratchet.py` | PASS ("complexity ratchet ok (578 functions)") | green |

### vulture detail (pre-existing, not a regression)

```
tests/test_torch_backend_registry.py:24: unused variable 'clean_registry' (100% confidence)
tests/test_torch_backend_registry.py:36: unused variable 'clean_registry' (100% confidence)
```

`clean_registry` is a pytest fixture consumed for side effect; vulture
flags the parameter as unused. Introduced by `63e220a` (`feat(mlopt-torch):
register() seats torch on BackendRegistry`), present since ~v0.1.0 —
**not** a v0.1.21 regression. Because `scripts/gates.sh` runs vulture
before the suite under `set -e`, the dev-lane suite stage never ran
inside gates.sh; the suite was run manually (below).

## Rust arm — `ci.sh` (cargo test + release certification leg)

**FAIL.** `cargo test --manifest-path packages/mlopt-experiments/Cargo.toml`
dies compiling `handl-egraph v0.1.0` pinned at git rev `b873c128`
(`packages/mlopt-experiments/Cargo.lock:103-105`):

```
error[E0027]: pattern does not mention field `attrs`
  --> .../egraph-0bf1110ca106b670/b873c12/src/term_bridge.rs:57
error[E0063]: missing field `attrs` in initializer of `handl_language::Term`
  --> .../term_bridge.rs:267
```

Root cause: cross-repo pin skew, **not** a graphopt code change. The org's
`[patch]` dev-link convention resolves `handl-language` to the local
sibling checkout `~/GitHub/handl-lang/kernel`, which gained
`Term::Op.attrs` in `a5fc402` (ADR-0173, 2026-11-15). egraph `main` already
carries the fix (`7eebc5c feat(term_bridge): map kernel Term::Op attrs
through the bridge (ADR-0173)`, local egraph HEAD `2b83e74`), but the
experiments lockfile still pins `b873c12`, which predates it. HEAD commit
`389e3f2` touched only `pyproject.toml`/`uv.lock` — the Rust arm was
already red before it; the mid-run `af46e8f` Cargo.lock refresh did
**not** bump egraph (still `b873c12`), so ci.sh remains red.

Fix (not applied — out of scope): `cargo update -p handl-egraph` in
`packages/mlopt-experiments` to pull egraph `main@2b83e74`+, or wait for
the egraph-side release/pin bump. Note `~/GitHub/handl-lang/kernel`
worktree is dirty (`M Cargo.lock`, `M crates/handl-value/src/nbe.rs`) —
another agent is in flight there; the patch resolves against that
working tree, so the Rust arm's result can shift with sibling state.

Also note: a bare `cargo check` on the same manifest *appears* to pass
only because the pipeline exit code was masked by `tail`; the fingerprint
logs in `.ci-target/graphopt` contain the same E0027/E0063 errors —
`cargo test` is the honest signal and it fails.

## Python suite

Suite size post-splits: **5,199 tests collected** (AGENTS.md's 6,867
figure is stale — substrate/game/discovery package-bound tests moved to
eqsat/gamekit/kit-discovery).

- `-m "not slow"` (gates.sh **dev lane**, 5,195 tests): attempted twice.
  Both runs were **SIGKILLed by earlyoom** at ~4–5% progress — the 11 GB
  host was simultaneously running another agent's full pytest in
  `/home/luis/tmp/kit-torch-work` plus a vitest worker storm
  (ktbhealth). journalctl: `earlyoom ... sending SIGKILL to process
  2304299/.venv/bin/python -m pytest tests/ -q -m not slow` (13:57) and
  `2327543` (14:05). Infra-blocked, not a test failure — no test
  reported FAILED before the kills.
- `-m "not e2e and not slow"` (**core lane**, 4,564 tests — the user's
  "fast lane": skips the 631 e2e engine-boundary tests and the 4 slow
  soak tests; `requires_cuda` tests do run — RTX 2050 present, CUDA
  available): _IN PROGRESS — see below._

<!-- PYTEST_RESULT -->

## Blocking assessment for D-2 (mlopt-torch removal)

- Python static gates: all green except pre-existing vulture noise in
  `tests/test_torch_backend_registry.py` (a file that will travel with
  mlopt-torch — the finding disappears with the split, or needs a
  `# noqa`-style fixture rename if the test stays).
- Rust arm (`ci.sh`): **red** on egraph/kernel pin skew — independent of
  the Python split but should be fixed (lock bump) before calling the
  repo fully green.
- pytest dev lane: could not complete under host memory contention;
  core lane result pending.
