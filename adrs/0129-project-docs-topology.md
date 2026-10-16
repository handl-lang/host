# ADR-0129: Project-docs topology for the target-repo split

## Context

ADR-0128 splits `platform` into ~16 repos (ports stay; modules move). The
code split has started — `handl-lang/lang` and `handl-lang/kit-canonical`
exist — but the docs have not followed. `project/` is still one tracked
200-file tree in `platform`: 138 ADRs, 10 tasks, notes, reports, RFCs,
`design/`, `parked.yaml`, `TODO.md`, `BACKLOG.md`, `principles.md`.

Three problems make "leave it as-is" wrong:

1. **Ownership.** A contributor scoped to `web/` or `stdlib/` has no
   project folder of their own; every doc lives in a repo they do not own.
2. **Build tree hygiene.** `platform`'s `main` carries docs alongside a
   2.9 GB `target/`; doc churn and code churn share one history.
3. **Addressability.** ADR numbers already collide in-tree (`0001`×2,
   `0002`×3, `0100`×4, `0107`×2, `0108`×2), and once files live in
   different repos a bare `ADR-0113` is ambiguous.

ADR-0128 §Phase 12 already says `docs/` becomes the published home. What it
does not say is **how** the existing `project/` tree is distributed, or how
docs are checked out alongside the code in each repo.

## Decision

1. **Every repo carries its own docs on a `project` orphan branch, mounted
   as a worktree at `<repo>/project`.** The branch root *is* the docs tree
   (flat: `adrs/`, `tasks/`, `notes/`, `TODO.md`, …), so the worktree path
   maps 1:1 to today's `platform/project/` layout.

   ```bash
   cd <repo>
   git worktree add --orphan -b project project
   cd project && <place docs at root> && git add -A \
     && git commit -m "docs: initialize project branch (ADR-0128)"
   cd .. && printf '/project/\n' >> .gitignore \
     && git commit -m "chore: ignore the project worktree"
   ```

   `main` stays a clean build tree; docs history is decoupled; the worktree
   is disposable (`git worktree remove project`). A `docs-mount` script
   mounts every repo's worktree side by side for a single docs workspace.

2. **ADRs live with the repo that owns the decision.**
   - **Ports, kernel, composition, engines, tooling, runtime** are
     `platform`'s, so their ADRs stay in `platform/project/adrs`. A module
     *implements* a port; it does not own the port's ADR.
   - **Repo-local decisions** (e.g. `0113` WebIDL/JS, `0116` `.hfc`, `0109`
     noise FFI, `0029` CLI) move to that repo's `project/adrs`.
   - **Cross-cutting ADRs are split.** The original number stays with the
     canonical/parent half in `platform`; the moved half gets a *new*
     number, allocated from the registry. Candidates: `0044`, `0077`,
     `0083`, `0092`, `0093`, `0107`, `0114`, `0128`.

3. **`project/adrs/INDEX.md` is the global ADR registry** —
   `NNNN | title | status | repo | path | supersedes/superseded-by`.
   Numbers are globally unique and allocated from the index; **never
   renumber** an existing ADR (the number is its identifier, cited in
   `README`, `AGENTS.md`, and code comments). This fixes the existing
   collisions going forward.

4. **`principles.md` is hosted once, in the `docs/` repo**, and linked from
   every repo's README / project branch. It is a project-wide constant, not
   a per-repo document.

5. **History is preserved with `git filter-repo --paths-from-file`**, one
   orphan branch per repo, produced from a clone of `platform` and fetched
   into the target repo as `project`. Copy-and-seed is not used.

6. **`platform` adopts the worktree model too**, and its `project` branch
   holds **only platform-owned docs** — every ADR/task/note assigned to
   another repo is removed from it in the same change.

7. **Until `docs/` exists as a repo, `platform` is the hub.** Each repo's
   `project/references.md` lists the canonical ADR ids it depends on
   (`id | title | url`) so a repo is self-describing without copying.
   Phase 12 (ADR-0128) later moves published ADRs into `docs/`.

## Consequences

- Each repo owns a `project` folder; a contributor's scope is now visible
  in the tree. `main` is a pure build tree.
- `platform/project` shrinks to its own docs; the `#/project/` gitignore
  hint in `.gitignore` becomes real.
- Cross-repo ADR references become ids + URLs instead of relative paths;
  `README.md`, `AGENTS.md`, `docs/*.md` and the `workspace-structure` /
  `write-adr` skills need that update.
- Splitting `0044`/`0077`/`0083`/`0092`/`0093`/`0107`/`0114`/`0128` is real
  editorial work: each produces a parent (kept) + one or more children
  (moved), all registered in `INDEX.md`.
- `git filter-repo` rewrites history; it runs on a **clone**, never on the
  live `platform` checkout, and its output is fetched into each target repo.
- The `split/*` scratch branches become redundant once the `project`
  branches exist; they are deleted after the seeding is verified.

## Status

Accepted.
