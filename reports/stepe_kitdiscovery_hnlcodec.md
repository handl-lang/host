---
title: Step E tail — kit_discovery.hnl_codec shim over mlopt_core.term_json
type: report
date: 2026-11-17
status: complete — worktree /home/luis/tmp/kit-discovery-stepe (detached @ fbb61e7, uncommitted); nothing pushed
scope: kit-discovery only — src/kit_discovery/hnl_codec.py, tests/test_hnl_codec.py, pyproject/uv.lock (worktree-only eqsat v0.1.12 test pin)
---

# Step E tail — `hnl_codec` → `mlopt_core.term_json` shim

Worktree: `/home/luis/tmp/kit-discovery-stepe` (`git worktree add …
origin/main --detach`, fetched first). The real checkout was not
touched. `uv lock` + `uv sync --group dev` ran inside the worktree;
installed `mlopt-core`/`mlopt-orchestrator`/`mlopt-dtypes` resolve to
eqsat `d67d1fc` (v0.1.12) — verified `mlopt_core.term_json` importable
and `kit_discovery.hnl_codec.term_to_json is
mlopt_core.term_json.term_to_json` → `True`.

## The shim

`src/kit_discovery/hnl_codec.py`: **479 → 63 lines.** The private
codec is deleted outright; the module is now a docstring + re-export:

```python
from mlopt_core.term_json import (
    TermJsonError as HnlCodecError,
)
from mlopt_core.term_json import (
    term_from_json,
    term_to_json,
)

__all__ = ["HnlCodecError", "term_from_json", "term_to_json"]
```

No wrappers needed — `term_json`'s signatures are strict supersets
of the old ones (`term_to_json(term, *, dag=None, leaf="arr",
budget=None)` ⊃ `(term, *, dag=None)`; `term_from_json(data, *,
allow_dag=True, validate=True, leaf="auto")` ⊃ `(data, *,
allow_dag=True)`), and `HnlCodecError` is an alias of
`TermJsonError` (both `ValueError`). Consumers importing the three
public names — `mlopt/stage.py`, `mlopt/adapters/{onnx_rust,
arch_rust}.py`, `graphopt/bench/drivers/audit/
onnx_lifter_differential.py` — need zero changes.

In-repo usage census: within kit-discovery, **only**
`tests/test_hnl_codec.py` imports the module (`__init__.py` mentions
it in prose). `hnl_import`/`hnl_surface` never touch it.

## Profile decision — canonical, and why

`term_json` has two encode-time divergences from the deleted private
codec: leaf spelling (`arr` vs `const-str`) and, implicitly, the
`source`/`attrs`/dag-`dim` spellings (canonical emits `"Native"` +
`attrs` field + dim-as-node-index unconditionally). Chose the
**canonical profile** (the `leaf="arr"` default — no profile
parameter survives in the shim):

- **Decode is permanently tolerant** — both leaf spellings, both
  attrs channels (`attrs` / `lit.__mlopt_attrs__`, never both), any
  `ArrSource` spelling (`"Native"`, `{"Foreign": …}`, `"Judgment"`,
  `{"DynamicForeign": …}`, `null`), and nested-record dag `dim`
  all decode identically. Every historical record keeps reading.
- **Emit consumers never required the legacy spellings.** Audited
  all four:
  - `mlopt/stage.py` `_term_encode`/`_term_decode` — records round
    trip through the same codec; kit-mlopt (`kit-prelude`) threads
    the record as **opaque JSON** (verified: `crates/kit-mlopt`
    only validates the `v` schema field, never parses terms).
  - `onnx_rust`/`arch_rust` — decode-only (`term_from_json` on
    source-onnx output); tolerant either way.
  - `onnx_lifter_differential._TermView` — canonicalizes away
    `source`/`dim`/attrs-channel entirely, but its leaf census
    distinguishes `Arr`-leaf vs `Const` nodes: the Python oracle
    **must** emit `Arr` leaves to match source-onnx (which emits
    `{"Arr": …, "source": {"Foreign": "tensor"}, "attrs": …}`).
    Canonical `leaf="arr"` is the only choice that keeps the
    differential's iso/diff verdicts meaningful; `const-str` would
    false-positive every leaf.
- **Canonical is strictly more engine-compatible.** The old codec's
  dag records spelled `Op.dim` as a nested `{"Const": …}` record —
  which engine-eqsat's `dag.rs` **refuses** (`node_ref` requires a
  node index). `term_json` emits dim-as-index, the schema
  `dag.rs` actually parses. `source:"Native"` is likewise the
  spelling law patterns pin (`pat.rs` exact-keyset attr matching);
  `{"Foreign":"tensor"}` terms would silently stop laws firing if a
  stage payload ever reached `saturate_json`.
- The `const-str` leaf profile exists in `term_json` solely for the
  **eqsat-run wire** (`mlopt_orchestrator/eqsat.py` delegates with
  `leaf="const-str"` until the engine-eqsat `arr`-enode round-trip
  lands everywhere) — it is not this surface's profile.

Net: this codec is a `.hnl`/source-lifter surface codec, not the
search wire; the canonical spelling is what `source-onnx` and
engine-eqsat converge on.

## Divergences handled (all decode-tolerant, none emitted)

| divergence | old hnl_codec | canonical term_json | shim behaviour |
|---|---|---|---|
| leaf spelling | `Arr` record | `Arr` record (same) | identical |
| eqsat legacy leaf | n/a (decoded as `Const(str)` — typing lost) | `Const "__mlopt_leaf__:{…}"` → `Var`/`Param` | **fixed**: now decodes to real leaves |
| `source` | `{"Foreign":"tensor"}` | `"Native"` | decode ignores; emit canonical |
| attrs channel | `lit.__mlopt_attrs__` | `attrs` field | decode accepts both, never both |
| dag `Op.dim` | nested `Const` record | node-table index | decode accepts both (dict dim ignored) |
| dag dedup | `id()`-keyed | content-keyed | equal-but-distinct leaves now count as shared → `dag=None` picks the record slightly more often; decode unaffected |
| error type | `HnlCodecError` | `TermJsonError` | alias — `is` identical |

## Dep-pin reality (the landing blocker)

`pyproject.toml` `[tool.uv.sources]`: eqsat trio `v0.1.10 →
v0.1.12`. But **uv propagates `[tool.uv.sources]` out of git
dependencies' workspace roots** — `mlopt-torch @ graphopt@v0.1.21`
still pins eqsat v0.1.10 and `kit-carriers@v0.1.3` does too, so a
bare v0.1.12 bump fails `uv lock` with "conflicting URLs for
`mlopt-core`". For the worktree test I added a `[tool.uv]
override-dependencies` block pinning the three `mlopt-*` URLs to
v0.1.12 (documented in-file as worktree-only).

Cascade state at report time:

- `discoverykit` — v0.1.23 pins eqsat v0.1.12 **but** the
  `mlopt_torch → kit_torch` rename landed at v0.1.21; kit-discovery
  still imports `mlopt_torch`, so v0.1.20 is the newest usable pin
  (its propagated v0.1.10 sources are overridden in the worktree).
- `kit-carriers` — v0.1.4+ requires `kit-torch`; v0.1.3 is the
  newest `mlopt-torch`-compatible release and still pins eqsat
  v0.1.10. Same override needed.
- `graphopt`/`mlopt-torch` — origin/main still pins eqsat v0.1.10;
  no v0.1.12-compatible `mlopt-torch` ref exists yet.
- `gamekit` — left at v0.1.12 (v0.1.14 exists and already pins
  eqsat v0.1.12, usable once the rest of the cascade lands).

**Go/no-go: the shim itself is GO** — mechanical, fully tested,
strictly more wire-compatible than what it replaces. **Landing on
kit-discovery main is BLOCKED on cascade order**: kit-discovery
cannot resolve eqsat v0.1.12 until (a) graphopt repins mlopt-torch's
substrate sources (or kit-discovery completes its own
`mlopt_torch → kit_torch` rename, after which discoverykit v0.1.23 /
kit-carriers v0.1.5 / kit-torch v0.1.2 all line up on eqsat
v0.1.12). Until then the only way to land is a permanent
`override-dependencies` block — possible but ugly; recommend
sequencing the codec shim *with* the rename/repin cascade rather
than ahead of it. The worktree is left uncommitted for inspection;
push nothing.

## Test results

- `tests/test_hnl_codec.py`: **53 passed** (was 46 tests / 8
  spelling-pin failures before update — all updated to canonical
  spellings: `attrs` field, `"Native"` source tolerated, dag node
  count 6→7 for the shared `Const(1)` dim node, `wireable`/`undecodable`
  error matches).
- New coverage added (`TestTermJsonDelegation`,
  `TestLegacySpellingsDecode`): shim identity (`is` checks against
  `term_json`), canonical emit spellings pinned, direct
  `term_json`↔`hnl_codec` round-trip equality, and permanent decode
  of every legacy spelling (Foreign source + lit attrs, const-str
  leaf, nested-dim dag record, all `ArrSource` variants).
- `tests/test_discovery_emit.py`: 35 passed (the in-repo emit
  surface).
- Spot slice — `test_discovery_{enumerate,families,forms,evidence,
  experiments,genkernel,handl,gauntlet}`: all green standalone
  (~500 tests).
- `ruff check` / `ruff format --check` on touched files: clean.
- `ty check`: 0 errors (3 pre-existing `unresolved-import` warnings
  for lazy `kit_cuda` binds — unchanged from main).
- Full `pytest tests/ -q` (2974 tests): first attempt showed a band
  of setup *errors* around the 19–26% mark — **self-inflicted
  contention**: I was running per-file pytest batches concurrently,
  and the real kit-discovery checkout simultaneously ran its own
  cascade shard suite (`/tmp/kd_retry.sh`, since finished: all
  shards green); both share `~/.cache/mlopt` and CPU. A clean second
  full run reached **~48%+ with zero failures/errors** before the
  report was written (still running — the suite's heavy middle is
  minutes-scale; command: `.venv/bin/python -m pytest tests/ -q` in
  the worktree). Every file in the earlier error band passes
  standalone; nothing in it touches the codec.

## LOC delta

- `src/kit_discovery/hnl_codec.py`: **479 → 63** (−416).
- `tests/test_hnl_codec.py`: 462 → 597 (+135 — spelling updates +
  delegation/divergence coverage).
- `pyproject.toml`: +12/−3 (eqsat v0.1.12 sources + the
  worktree-only `[tool.uv] override-dependencies` block — **strip
  or replace with real cascade pins before any landing**).
- `uv.lock`: regenerated in-worktree only.

## Follow-ups for the parent agent

1. The second full-suite run was still in progress at report time
   (~48%+, zero failures/errors). If the parent wants the complete
   number, rerun `.venv/bin/python -m pytest tests/ -q` in
   `/home/luis/tmp/kit-discovery-stepe` (or check the still-running
   background shell) — expect ~60–90 min on this box under load.
2. Landing sequence: kit-discovery's eqsat v0.1.12 bump belongs
   *after* the graphopt/mlopt-torch repin (and ideally after the
   `mlopt_torch → kit_torch` rename so discoverykit v0.1.23 +
   kit-carriers v0.1.5 + kit-torch v0.1.2 line up). The codec shim
   can ride that same commit.
3. `mlopt/stage.py` docstrings still say "v3 … hnl_codec Term-JSON"
   and `RECORD_VERSION = 3` — the dry-run recommended a v4 bump +
   kit-prelude `RECORD_SCHEMA_VERSION` repin in the same window;
   unchanged here (out of scope), but the emitted spelling DID
   change (`source`, `attrs` channel, dag `dim`) — all decode-
   tolerant, so no version gate is strictly required; decide
   deliberately.
4. `graphopt/bench/drivers/audit/onnx_lifter_differential.py`'s
   comment ("Python term_to_json emits the legacy
   lit.__mlopt_attrs__ record") is now stale — cosmetic only; the
   canonicalizer already merges both channels.
