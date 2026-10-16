# Task 0129 — Project-docs split (ADR-0129)

Distribute `platform/project/` across the target repos' `project` branches
and convert `platform` itself to the worktree model. Source of truth for the
assignment is `project/docs-split.yaml`; decisions are in
`project/adrs/0129-project-docs-topology.md`.

## Phase 0 — registry and conventions (no repos)

- [ ] **0.1 `project/adrs/INDEX.md`** — generate from `docs-split.yaml`:
      `NNNN | title | status | repo | path | supersedes/superseded-by`.
      Reserve the next free number for each split child (start at `0130`).
- [ ] **0.2 `docs-split.yaml` audit** — every path under `project/` appears
      exactly once (or as a `splits:` parent). Fail if a path is unmapped.
- [ ] **0.3 Skill updates** — `write-adr` gains "allocate from INDEX.md;
      never renumber"; `workspace-structure` gains the `project`-branch /
      worktree rule and drops the "`project/` is a folder in this repo" line.
- [ ] **0.4 Reference sweep** — `README.md`, `AGENTS.md`, `docs/*.md` and
      the skills cite `project/adrs/NNNN`; rewrite to id + URL for ADRs that
      leave `platform`, and to `project/` (the worktree path) for the ones
      that stay.

## Phase 1 — `platform` itself (pilot)

- [ ] **1.1** `git worktree add --orphan -b project project` in `platform`.
- [ ] **1.2** Move `platform`-owned docs to the branch root; `git rm` the
      docs assigned to other repos from `main`.
- [ ] **1.3** Add `/project/` to `.gitignore` on `main`; verify `cargo check
      --workspace` and the dogfood gate are unaffected.
- [ ] **1.4** Verify the worktree round-trips: `git worktree remove project`
      then `git worktree add project project` restores the docs.

## Phase 2 — `filter-repo` harness

- [ ] **2.1** `scripts/split-project-docs.sh` — reads `docs-split.yaml`,
      writes one `paths-<repo>.txt` per repo, and from a **clone** of
      `platform` runs:
      `git filter-repo --force --paths-from-file paths-<repo>.txt
       --path-rename project/:`.
      The rename puts the docs at the branch root (ADR-0129 §1).
- [ ] **2.2** Emit each result as an orphan branch `<repo>/project` and a
      `git bundle`, so a target repo ingests it with
      `git fetch <bundle> project:project` — never a working-tree copy.
- [ ] **2.3** Dry run: assert the union of all `paths-*.txt` equals
      `git ls-files project/` exactly once (the 0.2 invariant, executable).

## Phase 3 — seed the live repos

- [ ] **3.1** `lang/` — `project` worktree + `adrs/` (surface/language ADRs
      per manifest) + the `0044` child ADR + `references.md`.
- [ ] **3.2** `kit-canonical/` — `project` worktree + the `0092` child ADR
      (test-framework package ownership) + `references.md`.
- [ ] **3.3** Mount both with the `docs-mount` script; confirm a single
      editor session sees `platform`, `lang`, `kit-canonical` docs.

## Phase 4 — roll out with ADR-0128

Seed each repo's `project` branch in the same PR that extracts its code, so
the docs and the module land together:

- [ ] **4.1** `python/` (Phase 1), `c/` (2), `stdlib/` (3), `otlp/` (4),
      `web/` (5), `native/` (6), `dot/term/sexp/handl/` (7), `cli/lsp/` (8),
      `meta/` (9), `ffi/` (10).
- [ ] **4.2** For each: allocate the split-child numbers in `INDEX.md`
      before moving the parent's non-canonical half.
- [ ] **4.3** Delete the `split/*` scratch branches once every target repo
      has a verified `project` branch.

## Phase 5 — `docs/` hub (ADR-0128 Phase 12)

- [ ] **5.1** `docs/` becomes a repo; `principles.md` lands there once and
      every repo links to it.
- [ ] **5.2** Published ADRs move from `platform/project/adrs` to
      `docs/adr/`; `INDEX.md` paths update; repo `references.md` files point
      at the published URLs.
- [ ] **5.3** `docs-mount` + the docs site aggregate per-repo rustdoc and
      `.hnl` surfaces into one site.

## Verification

- `git ls-files project/` in `platform` == the manifest's `platform` set.
- Every repo: `git worktree list` shows `project`; `main` has `/project/`
  ignored; `cargo check --workspace` green.
- `INDEX.md` has no duplicate numbers and no entry pointing at a missing
  file.
