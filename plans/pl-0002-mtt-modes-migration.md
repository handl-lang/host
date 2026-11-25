+++
id = "PL-002"
title = "MTT modes migration — grades generalize to modes"
status = "open"
created = "2026-11-25"
owner = "devin"
priority = "high"
links = ["graded-arrows.md", "effects-as-modes.md", "syntax-decisions.md", "ADR-0119", "ADR-0120", "ADR-0153", "ADR-0203", "ADR-0207", "ADR-0208"]
+++

Migrate the graded-arrow surface (`A ~[G]~> B`) from the "grade" framing to the
**Modal Type Theory (MTT) modes** framing: the bracket payload is a *mode* — an
object of a mode theory (2-category) — and modes become the unified axis for
coeffect grades, with dimensions and effects as sibling mode-algebra
instances where the literature supports it. User decision (2026-11-25):
"MTT looks the best one, because it generalizes."

Design record: [`design/graded-arrows.md`](../design/graded-arrows.md) §0
(modes reframing — the grade→mode rename, dims-vs-modes relationship,
effects deferral, honest scoping). This plan supersedes the P0–P3 table in
that doc's §8; the semantic commitments in §6 (Hom stays purely
dimensional, grades/modes never enter `type.hom`'s index) stand.

## What MTT changes vs. the grade framing

- **Vocabulary**: "grade" → "mode" in prose. Identifiers stay for now:
  `graded_arrow` node kind, `attrs["grade"]` channel (`OP_GRADE_ATTR`),
  `Arrow.grade` record field, `grade`/`sort ... grades` declarations.
  Renaming serde-visible keys is M5-evaluated, not assumed.
- **Generalization**: a mode is any object of a mode theory — usage
  counts (QTT semiring), permission/effect rows (Granule), session
  protocols (ADR-0203), staging, cost. One mechanism, many algebras.
- **Dimensions are NOT modes**: `~>[n]` stays the globular cell index
  (`Hom(n, src, tgt)`, ADR-0153/0119). Modes annotate the 1-cell; dims
  index the globular structure. Strictly separate axes, separate node
  kinds, separate checkers paths — unchanged from the grade design.
- **Effects are a deferred mode-algebra instance**: MTT puts effect rows
  in the mode theory (the "effect mode"). `IO<sig>`/`handle`/`perform`
  stays canonical; unification is M4 assessment only.

## Honest scoping (read first)

- **In scope (P2-sized)**: mode *representation* — node kind, eval/type
  schemes, `Op` channel, well-formedness synth, algebra-dispatched
  composition (`mode_compose`), mode subsumption at use sites where
  annotations exist.
- **Research-grade / out of scope**: full MTT — mode contexts
  (`Γ ⊢ M : A @ m`), mode substitution, strictness/usage extraction,
  mode polymorphism (`forall m.`), modality bridges between modes,
  effects-as-modes unification. No shipped system enforces grades
  without a fixed algebra + constraint machinery; handl ships
  representation + well-formedness + one canonical default algebra,
  honest-bound fallback everywhere else — same posture `IO<sig>` took.
- **Blast radius**: zero `~[G]~>` sites exist in the corpus (all
  `~[…​]~>` payloads are dimensions — graded-arrows.md §1.4). Mode
  checking can only fire where users write `~[G]~>`, so the stable
  typechecker cannot regress on existing code.

## Verified current state (2026-11-25, `mtt-modes` worktrees at main HEAD)

- `~>[n]` spelling landed (host `7b77d14c`-era; `grammar.dim_arrow.bare.infix`
  mints the same `dim_arrow` node as legacy `~[n]~>`; both parse).
- Codemod `~[x]~>`→`~>[x]` done corpus-wide (~30 repos, 1,747 sites);
  stragglers: model files + one lib-cli site (grammar newline bug, fixed
  separately in tool-parse-pratt — NOT in the mtt worktree set).
- Kernel `45707b1` (on `main`, merged into `mtt-modes` base):
  `attrs["grade"]` channel on `Term::Op` — `OP_GRADE_ATTR`,
  `Term::with_op_grade`/`Term::op_grade`, serde round-trip +
  e-node-key safety (`kernel/crates/handl-calculus/src/term.rs` ~lines
  79–98, 292, 663–720). **No consumers yet.**
- `graded_arrow` does NOT exist in committed kit data at `a9ea4d32`.
  **In-flight uncommitted work on the MAIN host checkout** already drafts
  it: `ast.graded_arrow` (roles source/grade/target), `eval.graded_arrow`
  (`dim: 1`, grade child → `Arrow.grade` field), `type.graded_arrow`
  (`synth_graded_arrow` judgment — op does not exist yet), plus
  `handl-kit-arrow-lower` routing `graded_arrow` signature grades to
  `with_op_grade` instead of `Op.dim` (+ `tests/lower.rs`). Whoever owns
  that diff must commit it (or M1 ports it) before M1 builds on it.
- NBE has `dim_arrow` paths only: `synth_dim_arrow_judgment`
  (`dependent.rs` ~9157), `dep_synth_type` `dim_arrow` arm (~3346),
  `judgments.rs` ~369 dispatch, `infer_hom` boundary law (`checker.rs`
  ~622). No `graded_arrow`/`mode` arms anywhere.
- Model `typecheck_model.hnl` (8,317 lines): `synth_dim_arrow` ~7073,
  `cell_boundary_` ~2761, `cell_paste_` ~2810 (copies `grade` field
  verbatim — the seam where mode composition hooks in). No
  `grade_compose`/`mode_compose` anywhere (confirmed by grep).
- `handl-kit-arrow-lower`: `lower_op` signature split already drafted
  (in-flight diff above); legacy `graded_arrow` handling exists at
  ~558, ~1367–1388, ~2109, ~3726–3792 for the pratt dialect.
- `tool-parse-pratt` still mints legacy `graded_arrow` for
  `A ~[R]~> B` / `A ~ R ~> B` (`src/lib.rs` ~1439, tests ~4240–4375) —
  separate dialect, out of canonical-kit scope until M5.

## Worktree / merge strategy

Work happens in `/home/luis/GitHub/handl-lang-mtt/` — `mtt-modes` branches
in `kernel`, `engine`, `host`, `tool-typecheck-nbe`, `tool-typecheck-handl`;
all other siblings symlink to real checkouts. Docs/plan commits go to the
REAL `host/project` worktree (`/home/luis/GitHub/handl-lang/host/project`,
branch `project`) — never the mtt `host` checkout's `project/` submodule
path confusion: `project` is a linked worktree of the `project` repo.

- M1–M3 can land on `mtt-modes` branches **independently per repo** as
  long as each repo's gate is green; cross-repo consistency is enforced
  by the shared `Language` data snapshot + smoke suite, not by atomic
  merges. Merge order when ready: kernel (already done) → host yaml +
  arrow-lower → tool-typecheck-handl model → tool-typecheck-nbe parity.
- **Collision risk**: the model-parity agent is editing
  `typecheck_model.hnl` on main; the `~>[n]` newline fix is in
  tool-parse-pratt (symlinked — always main). M1/M2 agents must rebase
  `mtt-modes` onto main HEAD before starting and expect model-file
  conflicts; keep model edits additive (new `synth_*`/`mode_*` funcs at
  the funcs table end) to minimize them.
- tool-parse-pratt is needed for M5 (grammar/payload-kind flip) but is
  NOT in the mtt worktree set — add a `mtt-modes` branch there when M5
  starts, or coordinate the flip on main.

## Phases

- [ ] **M0 design lock** — this plan + `graded-arrows.md` modes reframing
  (§0, §11 resolutions). No code. Commits: project repo (this file +
  design doc). Verification: doc review only.

- [ ] **M1 mode representation** — land the `graded_arrow` kind end-to-end
  with zero grammar change (nodes arrive via legacy pratt dialect /
  hand-built ASTs only; `~[x]~>` still parses to `dim_arrow` for every
  payload — codemod-first sequencing, graded-arrows.md §9.3a).
  - Land or port the in-flight main-checkout diff: `ast.graded_arrow`,
    `eval.graded_arrow`, `type.graded_arrow` in
    `host/crates/handl-kit-prelude-lang/data/{ast,eval,types}.yaml`;
    `handl-kit-arrow-lower` `lower_op` grade-channel routing +
    `tests/lower.rs`.
  - `synth_graded_arrow` judgment op: `typecheck_model.hnl` funcs table
    (mirror `synth_dim_arrow` ~7073 — synth source/target, synth grade
    child as a term, emit `Arrow{inputs, grade: <grade type>,
    output, dim: 1}`) + register in the judgments map ~8257; NBE
    `synth_graded_arrow_judgment` in `dependent.rs` + `dep_synth_type`
    `graded_arrow` arm + `judgments.rs` dispatch entry.
  - Fixture sync: `cp` model → `tests/fixtures/typecheck_model.hnl`
    (drift-guard test enforces).
  - Repos: host, tool-typecheck-handl, tool-typecheck-nbe. Kernel: none
    (channel exists). Engine: none expected.
  - Verification: `cargo xtask gate` per repo; host
    `cargo nextest run --workspace -j 2 --profile fast` (memory-bound —
    see host AGENTS.md); `bash scripts/ci.sh smoke`; new unit tests for
    `synth_graded_arrow` + arrow-lower grade routing (lower.rs already
    drafted).
  - Merge: independent per repo; model + NBE must land together-ish
    (type.graded_arrow names a judgment both engines must know).

- [ ] **M2 mode algebra interface** — compose/unit/order as data.
  - Canonical default mode algebra as a Handl data value: record-row
    union for record modes, equality for named modes, numeric add for
    literal counts, else fresh meta + deferred diagnostic (the
    "honest bound" convention from `infer_hom`/`handle_sig_check`).
    Ship as data (a `mode.algebra.*` value in kit globals or a
    `Language` fragment — decide at implementation; ADR-0207 §9 wants
    algebras selectable per composition site, Grass precedent).
  - `mode_compose` judgment op in the model + NBE: invoked from
    `cell_paste_`/composition when either side carries a mode
    (`Arrow.grade` non-number / `attrs["grade"]` present); dims keep
    the existing `dim = k+1` path untouched.
  - Mode well-formedness diagnostics: grade/mode child must elaborate
    (resolve declared `grade`/`sort` names, arity of mode constructors
    like `Session(!T)`); undeclared names warn (open question §11.1
    resolution: resolve against declared grades first).
  - Repos: tool-typecheck-handl (model), tool-typecheck-nbe (parity),
    host (kit data for the default algebra value + any `eval`/`types`
    rows it needs). Kernel: none unless the algebra needs a Term form
    (avoid — keep it Handl data).
  - Verification: same gates; new tests exercising compose on each
    algebra instance + the meta fallback.
  - Merge: model + NBE together; kit data can land first (inert until
    consumers exist).

- [ ] **M3 mode subsumption / checking** — the actual grading semantics,
  scoped hard:
  - `A ~[m₁]~> B` usable where `A ~[m₂]~> B` expected iff `m₁ ≤ m₂` in
    the governing algebra (row width for records, equality otherwise).
    Unresolvable → pass through (symbolic-sig convention).
  - Enforcement points: application (argument mode vs parameter mode)
    and composition result vs annotation. ONLY fires on `graded_arrow`
    nodes — zero corpus sites, zero regression surface.
  - Repos: tool-typecheck-handl, tool-typecheck-nbe (parity is
    mandatory — the model is canonical, NBE is production).
  - Verification: gates + smoke; new `.hnl` fixtures exercising
    subsumption accept/reject per algebra.
  - Merge: together with M2's algebra (needs `order` to mean something);
    if the canonical order is too weak to be useful, M3 may degrade to
    "compose + well-formedness only" — decide with evidence, record in
    this plan.

- [x] **M4 effects-as-modes — assessment only (deferred)**
  - MTT places effect rows in the mode theory; `IO<sig>` is already row
    data. Feasibility question: can `IO<{…}>` be re-read as a mode
    annotation without breaking `handle`'s coverage/residual machinery?
    Assessment: [`design/effects-as-modes.md`](../design/effects-as-modes.md).
  - **Verdict: defer unification.** The categorical story exists
    (ADR-0158/Sanada — handlers as algebra homomorphisms; MTT
    modalities), and the sig *row* fits the M2 algebra interface
    (union/order ≈ existing record-row shapes), but coverage/residual-sig
    computation (`handle_sig_check`, `dependent.rs` ~10272–10480),
    row-polymorphic `IO(s)` metas, deep-handler codomain rewrites, and
    residual-trace values all live in the codomain-former machinery —
    replacing it needs ambient mode contexts (out of scope,
    research-grade) or a strictly poorer re-implementation.
  - Optional subset recorded in the assessment: register the effect row
    as a canonical mode-algebra instance in M2's registry
    (`compose` = row union, `order` = row-width subsumption) so
    `~[{io: S}]~>` annotations are *permitted* — gated on M2, zero risk
    to `IO<sig>` checking. Not attempted: moving
    coverage/residual/row-poly checking into modes.
  - `IO<sig>`/`handle`/`perform` remain the canonical effect mechanism.
    Revisit only if full-MTT mode contexts land or a second consumer
    (e.g. ADR-0203 sessions composing with effects) needs effect rows
    as modes.
  - Side finding (prerequisite for any effects-adjacent work):
    `synth_handle`/`synth_perform`/`check_call`'s perform branch are
    NBE-native judgment ops only — `typecheck_model.hnl` has no
    `synth_handle`/`synth_perform` (falls to "unknown judgment op").
    Port to the model before touching `IO`/`handle`/`perform`.

- [ ] **M5 transition cleanup**
  - Finish `~[x]~>` legacy payload-kind flip: after the `~>[n]` codemod
    stragglers land (model files + lib-cli site), flip the grammar so
    non-dimension payloads mint `graded_arrow` (`~[Addable]~>`,
    `~[Session(!T)]~>`), numeric stays `dim_arrow` sugar. Needs
    tool-parse-pratt work (payload-kind-conditional grammar row or
    desugar transform) — add `mtt-modes` branch there or coordinate on
    main; the newline fix in flight must land first.
  - Optional rename evaluation: `graded_arrow`→`mode_arrow`,
    `attrs["grade"]`→`attrs["mode"]`, `Arrow.grade`→`Arrow.mode`.
    Decision criterion: serde/`TermData` migration cost vs clarity.
    Recommendation on record: keep names, "mode" is the semantic term —
    revisit only if the mode vocabulary proves out.
  - Docs sweep: `syntax-decisions.md` (arrow table → modes), ADR-0153
    conformance note, `tokens.yaml` ~243 stale `op ... grade G` clause
    doc (open question §11.6 — recommend drop), corpus straggler codemod.

## Risks

- **In-flight collision (highest)**: uncommitted `graded_arrow` yaml +
  arrow-lower diff on main host; model-parity agent on
  `typecheck_model.hnl`; `~>[n]` newline fix in tool-parse-pratt.
  Mitigate: M1 agent first inventories uncommitted diffs, rebases
  `mtt-modes`, ports rather than duplicates.
- **NBE/model parity drift**: two engines must agree on `graded_arrow`
  synth + mode compose. The parity suites (`handl_typecheck_parity`,
  `dependent_structure`, `cubical_p*`) are the guard; run them, don't
  just `cargo test -p`.
- **Mode algebra ambiguity**: "canonical default algebra" could become
  a junk-drawer (row-union + equality + add + fallback in one dispatch).
  Keep the dispatch table small and explicit; anything unrecognized →
  meta + deferred diagnostic, never a guess.
- **Scope creep toward full MTT**: mode contexts, substitution, and
  strictness are explicitly out (see Honest scoping). Any phase that
  finds itself needing them stops and re-plans.
- **`dim: 1` convention**: graded arrows mint `dim: 1` so
  `cell_boundary_`/`infer_hom`/handler `dim_label` treat them as
  1-cells. If a future mode needs non-1-cell status, revisit — today
  modes annotate 1-cells only.
