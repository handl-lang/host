# Task: dead-code cleanup and the kernel boundary (ADR-0126)

Status: **in progress — C done; A next.** Plan agreed with the maintainer.

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

### B — `host-lsp` is a thin host, not a second LSP (ruling 3)

16 modules under `crates/host-lsp/src/handlers/` have zero test coverage but are
runtime-reachable (an editor reaches them). They are **not** dead — they are
unverified, and in some cases duplicating `plugin-lsp`.

This is an **audit, not a test-writing project**: for each module, is there an
equivalent in `plugin-lsp`? Duplicate → delete the `host-lsp` copy. Genuinely
host-specific protocol glue → keep. Needs a read of a few files to classify.

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
| `*_handl_plugin()` constructors (crypto, kv-hashmap, otlp, otlp-std, parse, serialize, tag, ip) | 8 | one `HandlLibraryPlugin` ctor per FFI library — identical in shape | **superseded**: libraries now register by `*Module` type, not via the `HandlLibraryPlugin` wrapper — verified for `crypto` (`kit-canonical` → `handl_plugin_lib_crypto::module::CryptoModule`) and `crypto_handl_plugin()` is called nowhere. One decision retires all eight |
| `plugin-compile-*` emit/ABI internals (`emit_program`, `emit_placeholder`, `dwarf_line_table`, `build_apply_native_call`, `expect_tag`) | 6 | backend internals; `dwarf_line_table` + the 0%-covered `emit/debug_info.rs` | **an unwired idea**: debug info for compiled output. Keep and record — do not delete |
| `handl-language` fragment builders (`with_faces`, `with_keyword_clauses`, `with_trailing_tokens`, `with_trivia`, …) | 6 | programmatic construction API for language data (ADR-0044/0081/0096) | **capability API** — the repo builds `Language`s from YAML, but `with_trailing_tokens` backs a real ADR-0096 feature. Keep |
| `host-lsp` internals (`hover_at`, `property_at_offset`, `cross_arrow_hover`, `invalidate`, `invalidate_dir`, `rest_of_line`) | 6 | hover variants + kit-resolver cache invalidation | **fold into workstream B** (the `host-lsp` vs `plugin-lsp` audit) |
| `plugin-engine*` / transform (`with_rule_source`, `run_pass_root`, `child_is_kind`, `remap_children_from`, `op_native_dim`, `fixpoint_trace`) | 6 | engine entry points; `op_native_dim` is a dimensioned `Term` ctor | **`op_native_dim` belongs to D (n-dim)**; the rest are engine API — keep |
| singletons (`is_enabled`, `has_plugin_id`, `from_arc`, `set_partial`, `with_tag`, `new_with_project`, `with_caps`, `with_factories`, `with_resolver_fallback`, `into_vm_error`, `invalid_argument`, `fixpoint_trace`) | 12 | one-offs across the workspace | case-by-case; none is urgent |

**Outcome:** no deletions. Six of the eight families turn out to be *live API or a
recorded idea*, not garbage — which is exactly the correction the maintainer
asked for. The two actionable families are the ADR-0081 classifiers (re-home) and
the `*_handl_plugin` constructors (superseded by the package table).

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
