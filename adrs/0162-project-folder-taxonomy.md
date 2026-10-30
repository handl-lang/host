---
title: '`project/` holds process artifacts; published docs live in the org `docs/`'
type: adr
date: 2026-10-28
updated: 2026-10-29
status: accepted
applies_to: all repos with a `project` branch; `docs` (the published hub)
---

# ADR-0162: `project/` holds process artifacts; published docs live in the org `docs/`

## Decision

1. **`project/` holds process artifacts only.** Explanations, tutorials,
   how-to guides and reference are published documentation and live in the
   org `docs` repo (each repo's `docs/site/`, vendored into the hub). A
   `project/docs/` directory is never created.
2. **The top-level set is closed**, identical in every repo:

   | entry | answers | lifetime |
   |---|---|---|
   | `adrs/` | *why* — the decision and what was rejected | append-only, permanent |
   | `design/` | *how* — mechanism, algorithms, state machines | until code is the spec |
   | `rfcs/` | *when* — the port plan to Handl | until the port lands |
   | `tasks/`, `TODO.md` | *who/what next* | consumed |
   | `BACKLOG.md` | *not now* — accepted, unscheduled | reaped quarterly |
   | `parked.yaml` | *retired* — ideas deliberately not built | permanent, with unpark trigger |
   | `notes/` | *what we found* — investigations, measurements | promoted or deleted |
   | `reports/` | *what we measured* — audits, sweeps | dated, immutable |
   | `retros/` | *what went wrong* — process post-mortems | dated, immutable |
   | `references.md` | *what we depend on* — other repos' ADR ids | maintained |
   | `DECISIONS.md` | *what was decided* — one line per rule | generated, never hand-edited |
   | `principles.md` | *org policy* — the standing principles (org `project` repo only) | maintained |
   | `docs-split.yaml` | *where things live* — assignment manifest | maintained |

   Anything else is a layout error, unless the repo declares it under
   `## Local entries` in `project/README.md` (one line: name — purpose).
   Core entries mean the same thing everywhere; local entries are a
   documented extension, not a free-for-all.
3. **One artifact answers one question.** A document that answers two is
   split; a document that answers none is deleted.
4. **Dated vs living.** `notes/`, `reports/`, `retros/` are dated artifacts and
   are never edited afterwards; the date lives in the frontmatter (`date:`),
   never in the filename (ADR-0163). `adrs/`, `design/`, `references.md` are
   current-truth and carry their creation/decision date in frontmatter only.
   Filenames are stable labels — a file is never renamed to carry a date.
5. **`design/`** is either named after the ADR it elaborates (`NNNN-slug.md`)
   or carries a `- Refs: ADR-NNNN, …` line under its title. Once the ADR's
   `## Enforcement` names the code that makes it true, the design doc may be
   deleted — git keeps it.
6. **A rejected design produces an ADR, never a note.** `notes/` is a
   one-cycle scratchpad: promote to `design/`/`adrs/` or delete.
7. **No copies across repos.** Cross-repo references are ids + URLs
   (ADR-0129 §7). A `project/` tree contains only its own repo's artifacts.
8. **One `project/` per repo, on its `project` branch** (ADR-0129 §1), mounted
   as a gitignored worktree. The org `project` repo holds **org-wide artifacts
   only** — cross-repo plans, org policy, the docs manifest — never a repo's
   own ADRs and never a staging copy of them.
9. `scripts/check-project-layout.py` enforces 1, 2, 7 and 8;
   `scripts/project-frontmatter.py` enforces the metadata schema (ADR-0163);
   `scripts/check-adrs.py` enforces the ADR side (ADR-0161).
10. `project/README.md` states this map in one screen and links this ADR.

## Why

There are four layouts in use today. `host/project` has
`adrs assets design notes reports tasks`; `kit-data-prelude/project` has
`adrs tasks references.md`; `graphopt/project` adds `plans`, `retros`,
`GOALS.md` and a `research_lab_notebook.md`; the org `project` repo adds
`c/ web/ plans/ defining-handl/ docs/`, three of which carry their own
`adrs/`. Consequences already paid: ADR-0150 and ADR-0151 exist twice
(embedded copies, in violation of ADR-0129 §7), nobody can predict where a
new document belongs so it gets written twice, and process documents sit in
the tree the docs site sweeps.

## Not doing

- **One flat directory with filename prefixes** — unreadable at 300 files and
  loses the "this is consumed, that is permanent" distinction.
- **Document the map inside `project/docs/`** — it would be published, then
  drift from the site's four quadrants.
- **Let each repo pick its own layout** — that is the current state.
- **Enforce by review culture alone** — already tried; the drift is the
  evidence.

## Consequences

- `host/project` lands clean: `assets/` folded into `notes/`, undated
  `notes/`/`reports/` take their git creation date in frontmatter (never in
  the filename), `design/` files given `Refs:` lines.
- `graphopt` declares `plans/` and `GOALS.md` as local entries (a research
  repo's workstream records and goal ledger), moves its lab notebook into
  `notes/`, and keeps `retros/` as-is.
- The org `project` repo holds only org-wide artifacts: the `c/` and `web/`
  trees moved to their own repos (`handl-lang/c`, `handl-lang/web`), each with
  a `project` branch and `plans/` staying behind as the cross-repo record.
- `project/docs/` is gone (published docs belong to the `docs` hub), as are the
  duplicated `plans/adrs/0150`, `0151`.
- Contributors get a single question to answer — "what kind of artifact is
  this?" — instead of a per-repo guess, and a declared escape hatch when the
  answer is genuinely new.

## Enforcement

- `host/scripts/check-project-layout.py` (closed set, copies, per-domain trees).
- `host/scripts/project-frontmatter.py` (ADR-0163 metadata, in every tree).
- `host/project/README.md` — the one-screen map.

## Revisit if

- A repo needs a ninth artifact type for more than two months; add it here by
  superseding this ADR rather than by adding a directory.
