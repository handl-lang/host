---
title: Step E tail — mlopt stage.py + emit.py codec migration dry-run
type: report
date: 2026-11-17
status: dry-run complete — worktree /home/luis/tmp/mlopt-stepe, branch stepe/mlopt-stage-codec @ 5bf44fa (detached-base origin/main 37f5902); nothing pushed; real checkout untouched
scope: mlopt only — pyproject eqsat repin, stage.py codec path, emit.py + adapter mlopt_core.ir→term repoints, tests
---

# Step E tail — mlopt stage/emit codec migration dry-run

Worktree: `git -C ~/GitHub/handl-lang/mlopt worktree add
/home/luis/tmp/mlopt-stepe origin/main --detach` → committed state
`37f5902` (note: **ahead of** the real checkout's local `main`
`9e2c53e` — origin/main carries newer deliver-train commits; the
dirty cascade pyproject + untracked `arch-*.safetensors` are not
present, and the uncommitted `kit_torch` renames are absent as
expected — the codec work does not depend on them).

## What migrated

| site | before | after | kind |
|---|---|---|---|
| `stage.py:_term_encode` | `kit_discovery.hnl_codec.term_to_json(term)` | `mlopt_core.term_json.term_to_json(term, leaf="arr")` | codec swap + explicit arr pin |
| `stage.py:_term_decode` | `kit_discovery.hnl_codec.term_from_json` | `mlopt_core.term_json.term_from_json` (defaults: `allow_dag=True, validate=True, leaf="auto"`) | codec swap; decode union unchanged |
| `stage.py:_ir_decode` | `from mlopt_core.ir import IR` | `from mlopt_core.term import IR` | mechanical |
| `stage.py` legacy path | `from mlopt_core.ir import term_from_data` | **unchanged** — the lemma-store codec lives in `ir.py` by design | stays on shim (correct) |
| `emit.py:115` | `mlopt_core.ir` Const/Op/Param/Var | `mlopt_core.term` | mechanical — emit walks view terms to `.hnl`, no codec work |
| `adapters/onnx_rust.py:67,580` | `mlopt_core.ir` types + `hnl_codec.term_from_json` | `mlopt_core.term` + `mlopt_core.term_json.term_from_json` | decode-target swap (plan's recommended direct import — drops the codec dep on kit-discovery) |
| `adapters/arch_rust.py:60,447` | same | same | same |
| `adapters/hnl.py:47` | `mlopt_core.ir` types | `mlopt_core.term` | mechanical; `hnl_import` untouched (still `kit_discovery`, still `mlopt_core.ir` internally — shim covers it) |
| `carrier_expand.py:39`, `deliver_game.py:565` | `mlopt_core.ir.Op` | `mlopt_core.term.Op` | mechanical isinstance/Op use, same class objects via shim anyway |

Docstrings/comments updated to name `mlopt_core.term_json` where they
described the codec (stage module docstring, RECORD_VERSION comment,
record-(de)serialization block, adapter module docstrings + registry
`description` strings).

## Wire-spelling audit (task 3 answer)

- **`emit.py` does no codec work** — it emits `.hnl` *surface*, not
  Term-JSON. The `ir → term` repoint is hygiene only; nothing about
  wire spellings changes.
- **`onnx_rust`/`arch_rust` are decode-only** on `doc["root"]`
  (source-onnx records: `Arr` leaves + `attrs` + `Foreign` source +
  nested dag `dim` — the hybrid spelling). `term_from_json` accepts
  all of it; verified live: legacy `hnl_codec` output and a hand-built
  `const-str` leaf payload both decode through `_term_decode`.
- **`_sanitize_param_names` (onnx_rust.py:325) stays raw-JSON and
  Arr-only** — it renames `{"Arr": {"value": {"__mlopt_leaf__":
  "param"…}}}` leaves before decode. Still correct *because*
  source-onnx emits `Arr`; a future const-str producer feeding these
  adapters would need the sanitizer extended (stepe_tail_plan §4
  constraint — unchanged by this migration).
- **`hnl.py` does surface parsing, not Term-JSON** — `hnl_import`
  builds view terms directly; its internal `mlopt_core.ir` use rides
  the shim until kit-discovery's own Step-3b repoint.

## RECORD_VERSION verdict: **CONFIRMED — stays 3**

`stage.py:139 RECORD_VERSION = 3` unchanged; `kit-prelude`
`RECORD_SCHEMA_VERSION = 3` needs no repin. Verified empirically:
new emit is canonical (`Arr` leaves, `source:"Native"`, `attrs`
channel, dim-index dag nodes) and `term_from_json` decodes every old
v3 spelling (`Foreign`, `lit.__mlopt_attrs__`, `const-str` leaves,
nested dag `dim`) plus v≤2 `term_to_data` payloads — the decode union
holds in both directions exactly as stepe_tail_plan §5 argued. The
codec swap changes spelling *inside* v3 payloads, not the record
schema. The dry-run's tentative "recommend v4" is refuted for the
landing decision: v4 would hard-fail every mixed-version deployment
(old kit refuses v4 input AND output) while buying zero decode
capability.

**const-str guard**: `_term_encode` pins `leaf="arr"` explicitly
(it is also the `term_to_json` default — the pin is documentation +
future-proofing, not a behavior change). `test_v3_payloads_emit_
canonical_term_json` asserts the emitted record JSON contains no
`"__mlopt_leaf__:"` const-str payload, no `"Foreign"`, no
`__mlopt_attrs__`, and `source == "Native"` on Op nodes.

## Dependency resolution — the real finding

Bumping only the three eqsat pins **does not lock**: uv honors
transitive `[tool.uv.sources]` through git deps (confirmed
empirically — the kit-discovery "dep sources are honored
transitively" comment is accurate). The still-v0.1.10-pinned consumers
conflict URL-for-URL. Resolution required:

- eqsat trio → `v0.1.12` (d67d1fc — verified in uv.lock)
- `gamekit` v0.1.12 → **v0.1.14**, `kit-host-tensor` v0.1.3 →
  **v0.1.4**, `discoverykit` v0.1.20 → **v0.1.23** (their sources
  already agree on eqsat v0.1.12)
- `kit-carriers`/`kit-cuda` stay v0.1.3, `kit-discovery` stays v0.1.2,
  `mlopt-torch` stays graphopt v0.1.21 — **no tag exists** with
  eqsat-v0.1.12 sources that still depends on `mlopt-torch`
  (kit-carriers/kit-cuda v0.1.4+ require `kit-torch` — the rename is
  cascade work, out of scope)
- `[tool.uv] override-dependencies` added forcing mlopt-core/
  orchestrator/dtypes + the three bumped git deps to their locked
  URLs — **marked NOT for landing** in the pyproject comment itself;
  the real fix is the consumer-cascade repin (graphopt, kit-discovery,
  kit-carriers, kit-cuda tags that pin eqsat v0.1.12)

## Test results (worktree `.venv`, eqsat d67d1fc)

- `tests/test_stage.py`: **30 passed** (incl. 3 new codec tests —
  canonical emit spellings, legacy `hnl_codec` v3 payload decode via
  the actually-installed v0.1.2 codec, `const-str` leaf decode)
- Emit/adapter slice (`test_emit`, `test_emit_surface`,
  `test_emit_embedding`, `test_hnl_source`, `test_onnx_rust_source`,
  `test_arch_rust_source`): **107 passed, 21 skipped** (skips are
  missing source-onnx binaries / HF models — environmental)
- Full suite `pytest tests/ -q`: **265 passed, 27 skipped** in 6.5 s
- `ruff check src`: clean; `ruff format --check src tests`: clean
- `ty check`: 9 diagnostics, **all pre-existing** — optional-extra
  imports (`transformers`, `huggingface_hub` not installed) +
  unrelated files (`scripts/whisper_*`, `adapters/{kokoro,whisper}.py`,
  `tests/test_emit.py`); zero diagnostics in touched files

## Incident notes for the parent agent

- **`/home` hit 100% disk mid-run** (188K free) — `uv lock` failed on
  git fetch. `uv cache prune` freed 7 MB; `uv cache clean` removed
  12.3 GiB of cache entries (safe — pure cache, auto-repopulates;
  other agents' venvs keep their hardlinked files). Space later
  recovered to ~6.7 G (another process freed ~6 G concurrently).
  mlopt's worktree `.venv` + `uv.lock` now exist at
  `/home/luis/tmp/mlopt-stepe` (~4-5 G) — delete the worktree if the
  disk pressure returns.
- A corrupt uv git-cache db (`refs/remotes/origin/tags` lock failure)
  was fixed by `rm -rf ~/.cache/uv/git-v0/db/472b8d3e2b300631`.

## Go/no-go for landing post-cascade

**GO, sequenced after the consumer repin cascade.** The codec swaps
are proven: stage records emit canonical `term_json` spellings with
`leaf="arr"` pinned, decode accepts the full legacy union, RECORD_VERSION
3 confirmed correct, and the full mlopt suite is green on real
eqsat v0.1.12. Landing checklist:

1. Land consumer-cascade repins first (graphopt/kit-discovery/
   kit-carriers/kit-cuda tags pinning eqsat v0.1.12 — kit-carriers
   v0.1.4+/kit-cuda v0.1.5 need `kit-torch`, i.e. post-D-1e).
2. Then land this branch **minus** the `override-dependencies` block
   and minus the now-unneeded transitive bumps (keep whichever dep
   bumps the cascade makes canonical — gamekit v0.1.14 /
   kit-host-tensor v0.1.4 / discoverykit v0.1.23 are probably fine to
   keep regardless).
3. D-2 interaction (stepe_tail_plan §8): these are small file-local
   edits; landing on mlopt main before the D-2 cut keeps the merge
   mechanical — prefer that if D-2 isn't imminent.

What stays on the shim permanently/by-design: `term_from_data`/
`term_from_json`-era lemma codec in `mlopt_core.ir` (stage v≤2 decode,
ADR-0178 D5), `kit_discovery.hnl_import` (surface parser, repoints
with kit-discovery Step 3b), and every `mlopt_core.ir` import inside
the pinned `mlopt-torch`/`kit-carriers`/`kit-cuda`/`kit-discovery`
packages themselves (their own repos' Step-3 passes).
