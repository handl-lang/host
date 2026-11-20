---
title: Step E tail — kernelopt codec-migration dry-run (eqsat v0.1.12)
type: report
date: 2026-11-17
status: dry-run complete — worktree branch committed, nothing pushed
scope: eqsat v0.1.10→v0.1.12 repin, Term-JSON codec-site audit, ir_bridge repoint, go/no-go
---

# kernelopt codec dry-run

Worktree: `/home/luis/tmp/kernelopt-stepe`, branch `stepe-codec-dryrun`,
commit `3fc0e7d` on top of `5a1679b` (local `main` — the unpushed
kit-torch cascade commit; `origin/main` is at `177c0d9`, so the
worktree was based on local `main` per instructions). The real
checkout's git state was not touched (its dirty `pyproject.toml`
 cascade bump remains as found).

## Verdict: GO — kernelopt rides the shim; zero codec migration needed

kernelopt has **no Term-JSON codec sites**. The full audit:

- `src/mlopt_kernel/ir_bridge.py` — the `mlopt_core.ir ↔ lib_kernel_ir`
  bridge is a *structural* term translator (walks `Op`/`Var`/`Param`/
  `Const`/`TensorType` view objects, emits `lib_kernel_ir.Term`/
  `Leaf`). No `term_to_json`/`term_from_json`/raw-JSON anywhere.
- The other 8 src files (`sink`, `board`, `scan`, `cost`, `play`,
  `laws`, `patterns`, `priorart`) import only view classes
  (`Const, Op, Param, TensorType, Var, IR`) — term walking/pattern
  matching, no serialization.
- All 6 bench files (`bench/discovery/kernel.py`,
  `bench/drivers/{overnight,screen/{priorart_verify,novelty_sieve},
  probes/{kernel_vocab_probe,kernel_scan_probe}}.py`) — same: view
  classes + `op_repr` (string repr) only. The `json.dumps` calls in
  `overnight.py`/`novelty_sieve.py`/`priorart_verify.py` serialize
  *result documents* (rows/ledgers), not terms. `overnight.py`'s
  `law_to_data`/`law_from_data` is `mlopt_core.laws.serialize` — the
  law-record codec, explicitly out of scope (stays per the plan's
  deprecation table).
- `tests/` — view-class imports only (left on the `ir` shim per plan).

So the codec question for kernelopt is exactly the plan's matrix row:
"view→`lib_kernel_ir` translator — repoint import only."

## What was done

1. **eqsat repin** — `mlopt-core`/`mlopt-dtypes`/`mlopt-orchestrator`
   `v0.1.10 → v0.1.12` in `pyproject.toml`; `uv lock` + `uv sync`
   resolved all three to `d67d1fc` (the term_json commit).
2. **Cascade** — transitive pins forced a wider bump (dep sources are
   honoured transitively): `kit-torch v0.1.0→v0.1.2`,
   `kit-carriers v0.1.3→v0.1.5`, `gamekit`/`benchkit v0.1.12→v0.1.14`,
   `discoverykit v0.1.20→v0.1.23`. Two blockers required workarounds:
   - `kit-discovery v0.1.2` (latest tag; its `hnl_codec` shim is
     still uncommitted) pins eqsat `v0.1.10`, gamekit `v0.1.12`,
     discoverykit `v0.1.20`, kit-carriers `v0.1.3`, **and still
     declares the pre-rename `mlopt-torch` runtime dep** → added a
     `mlopt-torch` source (graphopt `v0.1.21`, `packages/mlopt-torch`
     subdir — still on graphopt main per the plan) plus a
     `[tool.uv] override-dependencies` block forcing the
     v0.1.12-consistent graph. Same pattern as the kit-discovery
     worktree's not-for-landing override — **this pyproject is
     dry-run-only until a post-rename kit-discovery tag lands**.
   - Both `mlopt-torch` (pre-rename, used by `kit_discovery v0.1.2`)
     and `kit-torch v0.1.2` are installed side by side; kernelopt code
     only imports `kit_torch`/`kit_carriers`.
3. **ir_bridge repoint** — `src/mlopt_kernel/ir_bridge.py:61`
   `mlopt_core.ir → mlopt_core.term` (the plan's XS item; docstrings +
   the `UnsupportedOp` message updated to match — test regex
   `"not an mlopt_core"` still matches).
4. **Verification** — `pytest tests/` **242 passed** on eqsat
   `d67d1fc` (incl. `test_ir_bridge.py` 45/45, the lib-kernel-ir
   interop suite); `ruff check` + `ruff format --check` clean;
   `ty check` clean (needed a `../lib-kernel-ir` symlink in
   `/home/luis/tmp` for the worktree layout — env artifact, not a
   code issue).

## Go/no-go

**GO for the codec migration itself** — kernelopt needs nothing beyond
the `ir` shim; the `ir_bridge` repoint is landed in the dry-run commit
as the template for the remaining class-b files (deferred to the E-6
census per plan §3 — they ride the shim safely).

**NO-GO for landing the pyproject as-is** — the
`override-dependencies` + `mlopt-torch` source are cascade workarounds,
not a real resolution. The landable kernelopt eqsat `v0.1.12` bump
waits on **a kit-discovery tag that pins eqsat `v0.1.12` and drops the
`mlopt-torch` dep** (i.e. the hnl_codec-shim + repin work in
`/home/luis/tmp/kit-discovery-stepe` landing first). Once that tag
exists, kernelopt's landable diff is just the three eqsat pins (+
whatever cascade tags the new kit-discovery expects) — no overrides.

## Files changed (worktree commit `3fc0e7d`)

- `pyproject.toml` — eqsat v0.1.12 pins, cascade tag bumps,
  `mlopt-torch` source, `[tool.uv] override-dependencies` (dry-run).
- `uv.lock` — regenerated.
- `src/mlopt_kernel/ir_bridge.py` — `mlopt_core.ir → mlopt_core.term`.

Real checkout `/home/luis/GitHub/handl-lang/kernelopt` untouched;
nothing pushed.
