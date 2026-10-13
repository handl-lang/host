# Task: dead-code cleanup and the kernel boundary (ADR-0126)

Status: **A (part 1), C, E done; B and D measured.** Remaining: A part 2 (needs a
maintainer call — see below), B (a migration, deserves its own ADR), D (mechanical
re-enable). Plan agreed with the maintainer.

## The principle

ADR-0040: the kernel is only the composition primitives, and *everything is a
plugin*. ADR-0126 pins the boundary:

> Rust holds the **kernel** and **generic tooling**. Everything else is **Handl
> over a C ABI / SDK-gen contract**.

An unwired Rust crate is therefore an *idea in the wrong place*, not merely dead
weight. Retire the reification, keep the idea.

Maintainer's binding rulings (this plan is downstream of them):

1. The LSP owns **no** formatter of its own — formatting is the generic
   formatter's job (`plugin-ext-fmt`).
2. All tooling is Rust for performance and bootstrapping; it will be rewritten in
   Handl after bootstrap, and is **configured by Handl as data (plugins)**.
3. The LSP lives in `plugin-lsp`. **No duplication** in `host-lsp`.
4. **n-dim / n-morphisms are core** to the language. ADR-0119 stays.

## Evidence standard (learned the hard way)

Keep facts and guesses apart; never delete on a heuristic.

| Strength | Example | Action |
| --- | --- | --- |
| **Compiler-proven** | `never used` / `never read` from rustc; verified the live path elsewhere | delete |
| **Reachability-proven** | nothing depends on the crate (two independent scans) | retire via the ledger |
| **Heuristic** | "name appears once repo-wide"; "files look similar" | **no deletion** — census it instead |

An earlier draft listed 68 one-use public functions and four "near-clone" file
pairs as cleanup. Both are heuristics: `pub` is written on purpose, and differing
checksums prove only that files differ. They moved to workstream E, read-only.

## Workstreams

### A — the LSP owns no indenter (ruling 1) — PART 1 DONE

**Done — range formatting.** `crates/host-lsp/src/range_formatting.rs` went from
a 307-line hand-rolled line-based indenter to a ~85-line *projection* of the
canonical result: format the whole document with `format_source`, then emit an
edit for the selected lines that changed. Its tests were rewritten to assert
against `format_source` (the oracle) rather than hand-written strings, and the
divergence this cleanup had surfaced — "format selection" indenting input `hnl
fmt` leaves untouched — is gone: `cargo test -p handl-host-lsp` is 255 + 16
green.

**Pending — on-type formatting.** `crates/host-lsp/src/on_type_formatting.rs`
(295 lines) is the second hand-rolled indenter. It is *not* a mechanical
delegation: the feature exists to indent a **blank** new line as you type, and
`format_source` only rewrites lines that have content — so swapping it in would
quietly disable auto-indent. Two options, needing a maintainer decision:

1. give the generic formatter an "indent this blank line" entry point (the
   canonical indent logic already exists internally), then delegate; or
2. drop `textDocument/onTypeFormatting` from the LSP and rely on format-on-save.

The same question applies to `on_close_bracket` (re-indenting a line when a
closing bracket is typed) — option 1 covers both.

### B — `host-lsp` vs `plugin-lsp`: an accepted ADR, partially executed — AUDITED

**The decision already exists.** `project/adrs/archived/ADR-LSP.md` (**Status:
Accepted**) says:

> *"LSP features are implemented as separate plugins … `handl-tooling` exposes
> `ToolingHost`, which coordinates parsing, formatting, diagnostics, and the LSP
> plugin slots. `handl-host-lsp` dispatches LSP requests to `ToolingHost` … It
> does not know concrete parser or compiler internals. … Formatting and
> diagnostics are shared with the command-line tooling; they are not duplicated
> inside the LSP. … `handl-host-lsp` stays thin: document store, request mapping,
> and incremental sync."*

That is exactly the maintainer's ruling 1 and 3. So `host-lsp`'s 14,490 lines are
**a divergence from an accepted ADR**, and this workstream is *finishing that
ADR* — no new ADR is needed. `project/adrs/ADR-LSP-remaining.md` (**Proposed**)
is the live work item and names the next two features: **hover** and
**source-position resolution**.

**How far the migration got.** `ToolingHost` exists with plugin slots
(`with_completion_plugin`, `with_goto_def_plugin`, `with_document_symbols_plugin`,
`with_hover_plugin`, `with_signature_help_plugin`, `with_folding_range_plugin`,
`with_semantic_tokens_plugin`, `with_incremental_sync_plugin`), and **7 of 26**
handler modules dispatch through it: `completion`, `definition`,
`document_symbols`, `folding_ranges`, `hover`, `signature_help`, `workspace`
(plus `workspace_symbols`). The rest are hand-coded in `host-lsp` with no plugin
seam: `call_hierarchy`, `code_actions`, `code_lens`, `color`, `diagnostics`,
`document_highlight`, `document_link`, `execute_command`, `inlay_hints`,
`linked_editing`, `moniker`, `references`, `rename`, `selection_ranges`,
`type_hierarchy` (and the host-infrastructure ones: `initialize`, `lifecycle`,
`formatting`, `semantic_tokens`).

**Two divergences have already been closed** (workstream A, which is this ADR's
*"formatting … not duplicated inside the LSP"* clause): the second full-document
formatter (`c60fda0d`) and the hand-rolled range formatter (`f500a64f`).

**Correction to the earlier alarm.** The 16 zero-coverage files in
`handlers/` are *thin protocol glue* (26–64 lines each) delegating to top-level
feature modules that **do** have running `*_tests.rs` siblings. 0% there means
"no test drives the server protocol end to end", not "16 features are
unverified" — and that missing end-to-end test is arguably a **prerequisite** for
this migration, since it is the only thing that would catch a feature regressing
as it moves behind the seam.

**Recommendation:** finish `ADR-LSP-remaining.md` (hover next — it already has a
slot in `ToolingHost` and a placeholder plugin), and take the other un-migrated
features one at a time: each is either expressible as a `Language::lsp` rule
(move it) or genuinely host-specific (declare it and keep it). Nothing gets
deleted on the way.

**Prerequisite DONE.** The end-to-end safety net now exists:
`crates/host-lsp/src/handlers_e2e_tests.rs` drives **23** handlers with a real
document on disk and asserts each reply is a well-formed LSP response carrying the
request id — covering every one of the 16 previously-uncovered handler modules
(highlight, moniker, selection ranges, document link, references, colour,
linked editing, code actions, prepare/rename, prepare/sub/super type hierarchy,
call hierarchy incoming/outgoing, colour presentation, execute command, inlay
hint(s), resolve, signature help, workspace symbols, code lens). `host-lsp` went
from 255 to **278** tests, all passing, and no handler misbehaved — so the 0%
coverage was a missing test, not hidden breakage. A feature can now regress
loudly when it moves behind the seam.

### C — finish ADR-0119's cleanup half (supersession, hard evidence) — DONE

Landed in `refactor: finish ADR-0119's cleanup half`. Beyond the four JS
lowerers, the same supersession had left a `#[cfg(any())]`-disabled test module
in `term_data.rs` (458 lines, never compiled since the transition) — replaced by
an active round-trip test. A new gate (`no_silently_disabled_compilation` in
`crates/platform-tests/tests/wiring.rs`) now fails on any empty `cfg(any())`.

ADR-0119 states the proof/path forms are subsumed by the n-dimensional calculus:

> *"Dimension-2 arrows: a 2-cell is an arrow `f ~[2]~> g`. Rewrite rules are just
> `dim=2` arrows."*
> *"Proof and path forms (`Equiv`, `Path`, `Witness`, `Refine`, `Univalence`,
> `Glue`, `Fibrant`, `JElim`) are represented as `Arr`/`Op` terms at the
> appropriate dimension."*

Evidence: `Term::Path` / `Term::Equiv` exist **only** in
`plugin-engine/src/term_data.rs` and its own unit tests — no parser, no backend,
no construction from source. `CellId` / `Refine` / `JElim` were already deleted
by the same migration. `plugin-compile-js/src/lower/proof.rs` carries
`lower_path` / `lower_equiv` (and `_applied` forms) that are never dispatched:
the dispatcher matches only `Term::Rewrite`.

Therefore: delete `Term::Path` / `Term::Equiv`, the four never-dispatched
lowerers, and the `term_data` tests that only round-trip them.
**`Term::Rewrite` stays** — it is live in every backend and ADR-0119 merely
reinterprets it as a `dim=2` arrow.

### D — n-dim / n-morphisms — MEASURED, and it is not a feature project

Measured by running the ignored tests (`--ignored`). **Result: n-dim is
essentially implemented.** The 172 ignored tests are not blocked work; the
`#[ignore = "n-dim"]` labels are obsolete.

| File | Result |
| --- | --- |
| `arrow_model_delegate` | 8 / 8 pass |
| `arrow_model_parity` | 24 pass, 1 stale |
| `composite_model_parity` | 24 pass, 1 stale |
| `exec_model_parity` | 63 / 63 pass |
| `type_model_parity` | 22 pass, 1 stale |
| `value_model_parity` | 24 pass, 1 stale |
| `call_args_binding` | 1 / 1 pass |
| `op_handler_with_e2e` | 1 stale |
| `option_b_frontier_e2e` | 1 stale |
| `proc_notation` | 1 stale |

**166 pass; 6 fail, every one a stale assertion about something ADR-0119/ADR-0074
deliberately changed** — not a missing capability:

- `parity_cell_comp_v` (×3 files) expects a `{"__tag__":"Cell2", …}` record;
  ADR-0119 §4 folded `CellCompV`/`CellId`/`Whisker` into dimensioned `Arr`/`Op`.
- `type_model_cobind_is_graded_type` expects the pre-ADR-0119 `Arr` shape
  (`{"__tag__":"Arr","value":"Affine"}`); the implementation correctly emits the
  new one (`{"__tag__":"Arr","dim":{"__tag__":"Const","value":1},"value":"Affine"}`).
  The *implementation* is right — the test predates the `dim` field.
- `graded_arrow_lexes_parses_runs` uses `~[R]~>`, removed by ADR-0119 §1.
- `arrow_x_to_x_evaluates_to_closure` (`proc_notation`) is labelled *"function
  runtime value shape is not yet producing closure records"* — **that label is
  wrong**: closure records are minted (`handl-value` emits
  `{"__arrow__": true, "params": […], "body_def": …}`) and the test fails because
  `x ~> x` uses `~>` surface syntax that ADR-0074 removed.

**Revised scope for D:** delete the obsolete ignore labels, re-enable the 166
tests, and update the 6 stale assertions to the post-fold shapes. That is roughly
a day of mechanical work — *not* the "implement the n-dimensional calculus"
project the ignore labels implied. No ADR work needed; ADR-0119 stands.

**EXECUTED.** 169 of the 173 `n-dim` ignore labels were removed, so those tests
now run on every build — **all green** (`handl-runtime`: `arrow_model_delegate` 9,
`arrow_model_parity` 24, `composite_model_parity` 24, `exec_model_parity` 63,
`type_model_parity` 23, `value_model_parity` 24, `call_args_binding` 1,
`op_handler_with_e2e` 1, `option_b_frontier_e2e` 13; `handl-kit-data-canonical`:
`proc_notation` 6). Three stale expectations were corrected:

- `graded_arrow_lexes_parses_runs` and `op_handler_with_runs_end_to_end` used the
  pre-ADR-0119 graded surface `~Addable~>`; the current spelling is the bracketed
  `~[Addable]~>` (confirmed by `handl-kit-arrow-lower`, which documents a bare
  `var` in `~[N]~>` as a *named* dimension).
- `type_model_cobind_is_graded_type` expected the pre-ADR-0119 `Arr` shape; it now
  expects `dim` as well, which is what the implementation correctly emits.

Three labels remain, each with a **precise** reason instead of the blanket
`"n-dim"`, because they are genuine open questions rather than blocked work:

1. `arrow_parity_cell_comp_v`, `parity_cell_comp_v` (×2 files) — a **model-parity
   gap**: for a derived (`Term::Opaque`) record the Handl model evaluates it
   (`{"__tag__":"Cell2", …}`) while the Rust default model returns `Null`.
   Recording it rather than picking a side: ADR-0119 §4 says `CellCompV` is
   redundant once dimensions exist, so which model is "right" is a design call.

**`arrow_x_to_x_evaluates_to_closure` — resolved, and it corrected me.** Its label
claimed *"function runtime value shape is not yet producing closure records"*. I
first dismissed that as wrong (reasoning that `~>` was removed by ADR-0074), then
tested it: `~>` is alive — `grammar.arrow.infix` parses it into a `function` node
and `crypto.hnl` writes `name :: (params) ~> body` throughout — but neither
`x ~> x` nor `(x) ~> x` evaluates to a `__arrow__` record. A probe showed what it
actually returns:

```json
{"Op":{"args":[],"dim":{"Const":{"value":1}},"name":"id","source":"Native"}}
```

So **arrow literals evaluate to serialized `Term` records** (the arrow-lower
pipeline, ADR-0119/ADR-0120), and the identity folds to the `id` combinator. The
original label was substantively right, and the test now asserts the real
behaviour (`arrow_x_to_x_evaluates_to_the_identity_op`, re-enabled).

### E — census of the one-use public functions (read-only) — DONE

Scope: `pub fn` whose identifier appears **exactly once** in the whole repo
(definition only). Current count: **67 across 37 files** (the original sweep said
68; the retired crates accounted for almost none of it — the list was mostly
live-crate API surface). No deletions on this evidence alone: `pub` is written on
purpose, and a name used nowhere yet is not the same as a name that is useless.

The families, and where each belongs under the maintainer's rulings:

| Family | n | What it is | Verdict |
| --- | --- | --- | --- |
| `kit-canonical` classifiers (`is_*_kind`, `category_of`, …) | 11 | token classification as free functions over embedded YAML text | **misplaced**: ADR-0081 puts classification in `Language` as data (`token_in_category`). Check overlap — if covered, they are superseded; if not, move into `handl-language` |
| `platform` host/VM API (`with_globals`, `collect_vars`, `env_mut`, `get_plugin`, …) | 11 | the kernel's own public surface (ADR-0031/0040) | **kernel API** — keep, but likely wants an `api` module or `#[doc(hidden)]`; only a maintainer should shrink the kernel boundary |
| `host-tooling` (`run_cli`, `read_source_or_stdin`, `semantic_tokens_legend`, `with_*_plugin`) | 5 | CLI + LSP plumbing seams | **generic tooling**: check whether `host-cli` reimplements `run_cli` — if so, that is the ruling-2 "no duplicate" case applied to tooling, and `host-cli` should call it |
| `*_handl_plugin()` constructors (crypto, kv-hashmap, otlp, otlp-std, parse, serialize, tag, ip) | 8 | one `HandlLibraryPlugin` ctor per FFI library — identical in shape | **unused, and every library is registered by another path** — but *not* via one uniform replacement: `crypto`/`parse`/`tag` register a `*Module` type through `kit-canonical`; `kv-hashmap`/`otlp`/`otlp-std` go through `plugin-ext-kit::library_registry`; `serialize`/`parse` through `plugin-lib-stdlib::ops::*`. Each is `pub` API of its own crate, so retiring the eight is a per-crate decision, not one switch |
| `plugin-compile-*` emit/ABI internals (`emit_program`, `emit_placeholder`, `dwarf_line_table`, `build_apply_native_call`, `expect_tag`) | 6 | backend internals; `dwarf_line_table` + the 0%-covered `emit/debug_info.rs` | **an unwired idea**: debug info for compiled output. Keep and record — do not delete |
| `handl-language` fragment builders (`with_faces`, `with_keyword_clauses`, `with_trailing_tokens`, `with_trivia`, …) | 6 | programmatic construction API for language data (ADR-0044/0081/0096) | **capability API** — the repo builds `Language`s from YAML, but `with_trailing_tokens` backs a real ADR-0096 feature. Keep |
| `host-lsp` internals (`hover_at`, `property_at_offset`, `cross_arrow_hover`, `invalidate`, `invalidate_dir`, `rest_of_line`) | 6 | hover variants + kit-resolver cache invalidation | **fold into workstream B** (the `host-lsp` vs `plugin-lsp` audit) |
| `plugin-engine*` / transform (`with_rule_source`, `run_pass_root`, `child_is_kind`, `remap_children_from`, `op_native_dim`, `fixpoint_trace`) | 6 | engine entry points; `op_native_dim` is a dimensioned `Term` ctor | **`op_native_dim` belongs to D (n-dim)**; the rest are engine API — keep |
| singletons (`is_enabled`, `has_plugin_id`, `from_arc`, `set_partial`, `with_tag`, `new_with_project`, `with_caps`, `with_factories`, `with_resolver_fallback`, `into_vm_error`, `invalid_argument`, `fixpoint_trace`) | 12 | one-offs across the workspace | case-by-case; none is urgent |

**Outcome:** no deletions. Six of the eight families turn out to be *live API or a
recorded idea*, not garbage — which is exactly the correction the maintainer
asked for. Two families are candidates for a maintainer decision, and neither is
a bulk switch:

- the `kit-canonical` token classifiers: ADR-0081 puts classification in
  `Language` as data (`token_in_category`), so these are misplaced rather than
  dead — but first check how much `token_in_category` already covers;
- the eight `*_handl_plugin()` constructors: unused, and every library *is*
  registered, but through three different mechanisms (`kit-canonical` `*Module`,
  `plugin-ext-kit::library_registry`, `plugin-lib-stdlib::ops::*`). Retiring them
  is a per-crate call.

**A note on process:** this census replaced an earlier claim that the eight
constructors were "superseded — one decision retires all eight". Checking each
crate showed that was too glib (only three register a `*Module`). The same
happened twice more in this task (the "68 useless functions", the "16 unverified
LSP features"). Heuristics produce leads; each one needs its own verification
before anything is deleted.

## Order

**A + C → E (census) → B (audit) → D (measure, then implement).**

A and C are deletions resting on hard evidence. E and B are reading. D is the
feature and needs scoping first.

## Done so far (on `main`, pushed)

- `53a558bd` — recovered 5 silently-nested tests in the NBE synthesiser
  (pre-existing brace defect; they had never run).
- `2a68cfab` — retired 14 unwired crates (~2.9k LOC) behind
  `project/parked.yaml`; added the wiring gate
  `crates/platform-tests/tests/wiring.rs`; removed the vestigial duplicate
  `KitManifest` + `from_manifest` shim. Members 102 → 87.
- `c60fda0d` — deleted `host-lsp`'s superseded second formatter (318 lines +
  `TextEdit` + 9 tests), dropped a dead helper, cleared 22 unused imports.
- Six `parked/*-2026` tags pushed, so every retirement is one `git checkout`
  away.
