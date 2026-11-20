# Layout-law budgets fix — myco I-003 (eqsat joint-pass perf regression)

Date: 2026-11-16. Follows `eqsat_v0110_perf_check.md` (the measurement
this fix answers). Repos touched: **kit-eqsat** (canonical corpus),
**eqsat** (Python law tags + regenerated snapshot), **engine-eqsat**
(builtin-pack tag parity).

## Verdict

**FIXED and SOUND on the captured spec** — but not via the mechanism
the task brief assumed. `LAW_FORMAT = 2` records have **no
`rule_budgets` field**: budgets are a per-run `SaturateOptions`
map (`eqsat-run` spec `options.rule_budgets`, name → max NEW enodes a
rule may contribute) that the Python bridge derives from
`rules.tagged(EXPANSIVE)` at `optimize(symmetry_budget=…)`. The
corpus-side half of the fix is therefore the **`expansive` tag** on
the layout records — the only budget-relevant field a record carries.

With all 77 layout laws tagged `expansive`, the joint pass
(`_JOINT_DEFAULT = WITH_LAYOUT − SUBSUMED`,
`symmetry_budget=_CROSS_PAIR_SYMMETRY_BUDGET=512`) budgets every one
of them at 512, exactly like the 32 already-budgeted rules.

## Commits (all pushed to `main`)

| repo | commit | change |
|---|---|---|
| kit-eqsat | `b2eb3d8` | `data/laws/layout.json`: `"expansive"` added to `tags` on all 77 records (`["expansive","layout"]`, sorted per `law_to_data`). +77 lines, nothing else. |
| engine-eqsat | `818c9ed` | `src/builtin.rs`: `layout()` maps `.with_tag("expansive")` over the pack — required because `tests/contracts.rs::laws_corpus_files_load_and_match_the_builtin_packs` compares kit records to builtin `Law`s verbatim (incl. tags). |
| eqsat | `a23bcb3` | `laws/layout.py`: `R` helper default tags `(LAYOUT,)` → `(LAYOUT, EXPANSIVE)`; `eqsat_laws.json` regenerated via `scripts/emit_eqsat_laws.py` (524 records, +77 tag lines). |

## Before/after — wire-level A/B on the captured spec

Spec: `/tmp/tamper_spec_2.json` (the report's real 16-node joint
capture: 158 laws, `options.rule_budgets` = the 32 pre-existing
EXPANSIVE names at 512). "After" variant `/tmp/spec2_layoutbudgets.json`
= same spec + all 77 layout names at 512 — byte-identical to what
`optimize()` now sends (verified: `jd.tagged(EXPANSIVE)` covers all
77 `LAYOUT_RULES`; 110 budgeted names total in the joint set).

Binary: `engine-eqsat/target/release/eqsat-run` (`818c9ed`+parent —
the tag change is metadata-only, engine behaviour identical).

| | before (rep_t2.json) | after (rep_spec2_layoutbudgets.json) |
|---|---|---|
| wall | **82–132 s** (report; 131.9 s EQSAT_TIMING run) | **10.1 s** |
| iterations / stop | 9 / FixedPoint | 9 / FixedPoint |
| n_enodes | 8 930 | 6 444 |
| n_classes | 138 | 138 |
| extracted term | — | **byte-identical** (`term` JSON equal) |
| extraction cost | 6.0 | **6.0** |
| `fired` set | — | **identical** |
| `budget_suspended` | 6 rules | 8 rules (+`comm_add`, +`transpose_pull_add`) |

Layout-law budget spend confirms the mechanism: `transpose_pull_add`
694, `transpose_push_add` 274, `linear_from_matmul_t` 314,
`linear_to_matmul_t` 132, `matmul_transpose_rev_dd` 8 — the
previously-unbounded high-fire laws now suspend; the other ~70 spend
0–few (they were firing guard-check candidates, not merges).

**Soundness: PASS.** Extraction term, cost, fired set, iter/class
counts all identical; only enode/proof-edge counts dropped (the point
of the fix). Note the task's strict "e-node count must stay identical"
criterion cannot literally hold — budgets exist to bound enode
contribution — but the *extraction result* is unchanged, which is the
soundness that matters (`verify=True` still gates delivery).

Residual: 10.1 s vs the 0.35 s v0.1.6-applicable subset — the 158-law
set is genuinely ~2× the wire and 109 rules now carry budgets, so
joint searches remain ~29× the old subset cost. If that still hurts,
the report's mitigation #4 (a "joint-safe" preset) is the next lever;
this fix removes the *unbounded* term of the blowup.

## Python-engine parity — no divergence introduced

`rule_budgets` is computed once in `optimize.py:906-913` from
`rules.tagged(EXPANSIVE)` and passed to `eg.run(...)` for **both**
engines; `mlopt_core.egraph.core.EGraph.run` implements the identical
max-NEW-enodes semantics (`core.py:1596`, stats report
`rule_budgets`/`budget_suspended` the same way). Tagging the Python
`Rewrite`s therefore budgets layout laws on `engine="python-egraph"`
and `engine="eqsat"` identically — the engines cannot diverge on this
field because neither reads it from the records.

Behaviour-change caveat (intended): any caller passing layout rules
with the default `symmetry_budget=2048` (non-joint paths) now gets
budgets where they were previously unbounded. Same cap semantics as
the symmetry laws; extraction on other terms *could* differ where a
layout law's contribution exceeded the cap — that is the designed
trade-off the tag documents, and `verify=True` gates delivery.

## Verification run

- kit-eqsat `cargo test --workspace` — pass (incl. `corpus_drift`).
- engine-eqsat `cargo test` — pass (the contracts test needed the
  `builtin.rs` tag parity commit; confirmed failing before, green after).
- eqsat `pytest tests/test_law_bank_corpus_parity.py
  tests/test_eqsat_laws_corpus.py` — 8 passed.
- eqsat `pytest tests/ -k "layout or law or ruleset or tags" -m "not
  slow"` — 354 passed, 1 skipped.
- `ruff check`/`format --check` on `layout.py` — clean;
  `cargo fmt --check` engine-eqsat — clean.

## Consumer note

kit-eqsat and engine-eqsat are `branch = "main"` git deps (+ path
patches to sibling checkouts) — pushed commits reach consumers on next
`cargo`/`uv` resolve. **eqsat is tag-pinned** (`v0.1.10` etc.): the
Python-side half (`a23bcb3`) is on `main` but consumers won't see it
until the next eqsat tag/re-pin — until then the corpus tag is inert
(Rust stores `rule.tags` but never reads it for scheduling; the
budget map comes only from the Python side). The full fix activates
when graphopt/mlopt re-pin eqsat past `a23bcb3`.

Artifacts: `/tmp/spec2_layoutbudgets.json`,
`/tmp/rep_spec2_layoutbudgets.json` (not committed).
