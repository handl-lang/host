# engine-eqsat post-I-003 verification

Date: 2026-11-16. Repo: `/home/luis/GitHub/handl-lang/engine-eqsat` @ `818c9ed`
("builtin: tag layout() laws expansive, matching kit-eqsat b2eb3d8"), on top of
`dd4815e` ("dag: emit {"Arr": …} leaf records for canonical 'arr' enodes").

Scope: verify three recent landings — (1) arr-leaf round-trip `dd4815e`,
(2) kit-eqsat corpus expansive tags `b2eb3d8` picked up via `branch = "main"`
dev-dep, (3) eqsat v0.1.11/v0.1.12 Python-side changes (confirm not consumed
by the Rust engine). Plus the `rule_budgets` mechanism check: does the
corpus `expansive` tag do anything Rust-side, and does `eqsat-run` honor
`options.rule_budgets`?

## 1. Gates — all green

| Gate | Result |
|---|---|
| `cargo test` | **PASS** — 144 tests, 0 failures across 7 binaries: lib unittests 11, eqsat-run bin 0, `tests/contracts.rs` 20, `tests/eqsat.rs` 67, `tests/shape.rs` 21, `tests/stage4.rs` 24, doctest 1. |
| `cargo fmt --check` | **PASS** (clean, no diff). |
| `cargo clippy --all-targets -- -D warnings` | **PASS** — `Finished dev profile`, zero lints. Only pre-existing cargo warnings about unused `[patch]` entries in sibling manifests (not this crate's lints). |

Not run: `cargo doc` (not requested; `ci.sh` layout covers it separately).

## 2. Corpus resolution — kit-eqsat b2eb3d8 confirmed

- `Cargo.toml:32` declares `handl-kit-eqsat = { git = "…/kit-eqsat", branch = "main" }`
  as a dev-dep; `Cargo.toml:54-55` `[patch]` table dev-links it to the sibling
  checkout `../kit-eqsat/crates/handl-kit-eqsat`.
- `cargo tree -p handl-kit-eqsat` resolves to
  `handl-kit-eqsat v0.1.0 (/home/luis/GitHub/handl-lang/kit-eqsat/crates/handl-kit-eqsat)`.
- That checkout's HEAD **is** `b2eb3d8` ("corpus: tag the 77 layout laws
  expansive (I-003 perf fix)", 2026-11-16 15:45 -0300).
- Corpus proof: `data/laws/layout.json` holds **77 records, all 77 tagged
  `"expansive"`** (verified by decoding the JSON). `"expansive"` appears in 7
  of the 10 packs (simplification, categorical, prior_art, attention,
  pointwise_factor, conv_folds, layout). Commit message notes the LAW_FORMAT=2
  record carries no budget field — the tag is the corpus-side half; the
  Python bridge derives `rule_budgets` from EXPANSIVE-tagged rules
  (`optimize.py` `symmetry_budget`).

## 3. Arr-leaf round-trip — works in debug, release binary is STALE

Spec run through `eqsat-run` (stdin JSON): a `dag.program` whose node table
contains two canonical `{"Arr": {"dim": …, "value": {"__mlopt_leaf__": …}}}`
leaf records under an `add` op — the hnl_codec leaf spelling.

- **`target/debug/eqsat-run`** (built 2026-11-16 15:44, post-`dd4815e`):
  `ok: true`, extracted term carries both `{"Arr": …}` records verbatim —
  `dim`, `value.__mlopt_leaf__`, `name`, `shape` all intact. n_enodes=4,
  n_classes=4, cost 3.0.
- **`target/release/eqsat-run`** (built 2026-11-16 **04:26**, ~6h before
  `dd4815e` at 10:43): **fails** — `{"ok":false,"error":"bad term: dag
  output: enode attr \"value\" has no kernel Term spelling"}`, exit 1.
  This is exactly the bug `dd4815e` fixed; the release binary predates it.
  **Action if the release binary is consumed anywhere: rebuild
  (`cargo build --release`) or treat it as stale.**

In-tree coverage: `tests/eqsat.rs` `dag_program_arr_leaves_round_trip_tree_wire`
and `dag_program_arr_leaf_survives_dag_output` both pass (the latter exercises
the dag-table output spelling where `Arr.dim` is an inline tree record).

## 4. Budget mechanism — verdict: engine is correct, relies on the Python caller

Traced end-to-end and exercised live:

- `src/engine.rs:96` — `SaturateOptions.rule_budgets: BTreeMap<String, usize>`
  ("rule name -> max NEW enodes the rule may contribute").
- `src/bin/eqsat-run.rs:124-129` — decodes `options.rule_budgets` from the spec.
- `src/engine.rs:199` — forwarded into `handl_egraph::RunOptions.rule_budgets`.
- Enforcement lives in the egraph substrate
  (`/home/luis/GitHub/handl-lang/egraph/src/egraph.rs:1750` — per-rule budget
  lookup during saturation; `:1719` seeds budget state; stats carry
  `rule_budgets`/`budget_suspended`).

Live check (debug binary, `comm_add` law record lifted verbatim from the
kit-eqsat `simplification` pack, term `add(x, w)` over two Arr leaves):

- No budgets: `fired: ["comm_add"]`, n_enodes 4→5, `budget_suspended: []`.
- `"rule_budgets": {"comm_add": 0}`: `fired: []`, `budget_suspended:
  ["comm_add"]`, stats echo `rule_budgets: {"comm_add": 0}` — the rule is
  suspended before contributing enodes.

**On the `expansive` tag:** it is *metadata only* Rust-side. `Law.tags`
(`src/law.rs:54-56`) is "domain metadata carried opaquely, like
`Rewrite::tags`" — nothing in `src/` reads tag values to derive budgets or
alter firing. The corpus tag flows into `Law.tags` and is inspectable, but
**no Rust-side consumer turns `expansive` into a `rule_budgets` entry**.
That derivation is Python-side (`mlopt_orchestrator.optimize` /
`symmetry_budget`, per the b2eb3d8 commit message and the task brief), which
then sends `options.rule_budgets` in the spec — exactly the path verified
above.

**Verdict: engine-eqsat needs no change.** It correctly exposes the budget
seam (`SaturateOptions.rule_budgets` → `RunOptions.rule_budgets` →
`budget_suspended` stats) and honors caller-supplied budgets. The
`expansive`-tag → budget mapping is deliberately a caller concern; the Rust
engine treats tags as opaque provenance. If a future requirement is
"engine derives budgets from tags," that would be a new feature, not a bug.

## 5. eqsat v0.1.11/v0.1.12 Python-side changes — confirmed not consumed

`hnl-engine-eqsat`'s entire dep set is `handl-egraph`, `hnl-engine`,
`serde_json` (+ dev-dep `handl-kit-eqsat`) — `Cargo.toml:14-32`. There is no
dependency on the Python `eqsat` repo's crates; the Python releases touch
`mlopt_orchestrator`/`mlopt_core` code that this crate never links. The only
shared surface is the LAW_FORMAT=2 corpus (kit-eqsat data) and the JSON spec
protocol, both verified above.

## Summary

- Gates: test/fmt/clippy all pass (144 tests).
- Corpus: dev-dep resolves to kit-eqsat @ `b2eb3d8`; all 77 layout laws
  carry `"expansive"`.
- Arr-leaf: round-trips correctly through the **debug** `eqsat-run`;
  **release binary is stale** (pre-`dd4815e`, fails on `{"Arr": …}` leaves) —
  rebuild before any release-binary use.
- Budgets: `options.rule_budgets` honored end-to-end with `budget_suspended`
  reporting; `expansive` tags are opaque metadata Rust-side — budget
  derivation correctly lives in the Python caller. No engine change needed.
