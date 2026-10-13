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

### A — the LSP owns no indenter (ruling 1)

Delete the hand-rolled indentation family and route all formatting through the
canonical engine:

- `crates/host-lsp/src/range_formatting.rs` (own line-based pass)
- `crates/host-lsp/src/on_type_formatting.rs` (own line-based pass)
- the helpers they duplicate

Same move as `c60fda0d`, which already deleted `host-lsp/src/formatting.rs`'s
318-line second formatter; the live bridge is `format_document_json` →
`plugin-ext-fmt`. Tests currently assert the hand-rolled behaviour and must be
re-pointed at the canonical engine.

Found while working: the range formatter **disagrees with `hnl fmt`** (it indents
input the canonical engine leaves untouched), pinned by
`range_format_diverges_from_canonical_formatter`.

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

### D — implement n-dim / n-morphisms (the real feature)

ADR-0119 status: *"Proposed — design finalized during review; ready for
implementation."* 172 tests carry `#[ignore = "n-dim"]`; the reason is *"function
runtime value shape is not yet producing closure records"*.

**Measure before committing.** Half the blocker may be the parity harness rather
than missing runtime code; `handl-runtime/tests/arrow_model_*` and
`*_model_parity` are the suite to interrogate first. Deliver a size estimate
before any implementation.

### E — census, not cleanup (read-only)

Group the 68 one-use public functions into **families** and state, per family,
where it belongs under the rulings above (kernel / generic tooling / Handl data /
ledger). No deletions. Produce one page, not 68 line items.

Families already visible:

- 11 token classifiers in `kit-canonical` (`is_bool_kind`, `is_string_kind`,
  `is_operator_kind`, …) — ADR-0081 says classification belongs in `Language` as
  data (`token_in_category`); `AGENTS.md` flags this too. Misplaced, not dead.
- 7 `*_handl_plugin()` constructors — one per FFI library, identical in shape;
  `plugin-lib-stdlib/src/packages.rs` is where libraries register now.
- `host-tooling`'s `run_cli` / `read_source_or_stdin` / `with_*_plugin` — looks
  like CLI plumbing `host-cli` may be reimplementing.

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
