---
title: Step E tail — discoverykit codec census
type: report
date: 2026-11-19
status: census complete — no class-b codec sites; verdict "rides shim"; no source changes
scope: Term-JSON codec census of discoverykit against eqsat v0.1.12 (`d67d1fc8`)
---

# Step E tail — discoverykit codec census

Follows `stepe_tail_plan.md`, `stepe_leafkits_codec.md`, and
`stepe_kitdiscovery_hnlcodec.md`. Repo:
`/home/luis/GitHub/handl-lang/discoverykit`, worked directly on
`main` (clean tree, no worktree needed). Pin check:
`pyproject.toml:37` already carries `mlopt-core … tag = "v0.1.12"`
(lock resolves to `d67d1fc8d17f` — the ADR-0178 E-2 commit that
adds `mlopt_core.term` + `mlopt_core.term_json` and turns
`mlopt_core.ir` into a shim). `uv sync --group dev` was run once:
the venv had a stale pre-`term_json` mlopt-core (`414ec4e`); after
sync, `mlopt_core.term_json` imports and `ir.Op is term.Op` →
`True`.

**No code was changed — the honest result is "rides shim."**

## Census

Grep of `packages/discoverykit/src` + `tests/` for
`term_to_json` / `term_from_json` / `term_json` / `json.dumps` /
`json.loads` / `dag.program` / `hnl_codec` / `term_to_data` /
`term_from_data` / `cert_to_data` / `__tag__` / hand-rolled
`{"op": …, "args": …}` term dicts.

Every term-serialization site delegates to an `mlopt_core`
codec — none hand-rolls a wire:

- **`mlopt_core.laws.serialize`** is the store codec everywhere:
  - `evidence.py:642,646` — `alpha_key` + `object_to_data` into the
    `lemmas` sqlite `law_json` column; `evidence.py:711,765` —
    `object_kind` / `object_from_data` on read-back;
    `evidence.py:579-590` — `rulecache._enc_binding` for guard-spec
    substitution maps (its own codec, unchanged by E).
  - `cellstore.py:113,121` — `object_from_data` rebuild.
  - `antiunify.py:1128` — `law_to_data` for mint records.
  - `object_synthesis.py:1681-1683` — `object_to_data(obj.rule,
    cert=…, kind=…)`.
  - `handl.py:54-57,774,852` — `object_to_data` into the handl
    fragment row; `law_from_data` on ingest.
  - `machine_pack.py:58`, `proposal_core.py:99` — `missing_hooks`
    and friends (record-shape helpers, not wires).
- **`mlopt_core.egraph` cert codec** — `lemma_cert.py:80-81,379-380,
  469` and `evidence.py:918-924` use `cert_to_data` /
  `cert_from_data`; internally those ride `term_to_data`.
- **`mlopt_core.laws.cond`** — `object_synthesis.py:945`,
  `dspec_synth.py:52` use `derive_from_data` / cond specs.
- **Non-term JSON** (records/specs, correctly untouched):
  `handler_synth.py:1997-2012` `body_to_data`/`body_from_data`
  encode *handler grammar descriptors* (tuples of op-name specs —
  `lawdata/laws/handlers.py` rows like `{"args": ("X",), …}`), not
  IR terms; `domain_spec.py` spec JSON; `verifier.py:803`,
  `proposal_core.py:1251` report payloads; `coherence*.py`
  `_jsonable` report dumps.
- **`op_repr`** appears in ~20 sites (`antiunify`, `verifier`,
  `meta_arena`, `law_order`, `object_synthesis`) — all for display,
  equality keys, or report fields; none is reparsed back into a
  term (no `term_from_str`/`parse` counterpart exists or is needed).

## Why no migration to `term_json`

`term_json` is the **kernel Term-JSON envelope** (concrete terms,
shape/const payloads). The lemma-store wire is deliberately a
different dialect: eqsat v0.1.12's `ir.py` docstring (ADR-0178 D5,
option B) states `term_to_data`/`term_from_data` *stay* in `ir.py`
because the `{"__tag__": "dag.program", "nodes": …}` /
`{"op", "args", "attrs"}` spelling carries **metavariable leaves**
(`{"mvar": …}`) that patterns need and `term_json` does not speak.
discoverykit's stores are exactly that — law patterns, minted
records, certs — so `laws.serialize` → `ir.term_to_data` is the
correct, already-shimmed path. Same conclusion as
kit-host-tensor/kit-carriers.

## Verification

- `uv sync --group dev` — refreshed stale mlopt-core to the pinned
  `d67d1fc8`; `term_json` importable.
- `.venv/bin/python -m pytest tests/ -q` — **159 passed, 1 skipped**
  (0.63 s; the package-bound suite is small — heavy exercise runs
  through graphopt's cross-stack suite per AGENTS.md).
- `.venv/bin/ruff check packages` — clean; `ruff format --check` —
  39 files already formatted.
- `.venv/bin/ty check` — 0 errors; 120 `import-not-found`
  *warnings*, all the documented lazy-seam imports
  (`kit_discovery.*`, `kit_torch.*`, `torch`, `bench.*`) that ty
  cannot resolve from this repo alone.

## Verdict

**GO — lane closed, honest no-op.** discoverykit's 17 class-b src
files ride the `ir.py`/`laws.serialize` shim exactly as
kit-host-tensor and kit-carriers did; there is no `term_json` /
`dag.program` / `hnl_codec` work to migrate. The only action taken
was a `uv sync` to pick up the repinned eqsat commit — no source
edits, no commit, nothing pushed.
