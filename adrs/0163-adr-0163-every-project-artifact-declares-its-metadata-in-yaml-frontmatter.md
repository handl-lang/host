+++
id = "ADR-0163"
title = "ADR-0163: Every `project/` artifact declares its metadata in YAML frontmatter"
status = "open"
priority = "medium"
+++

---
title: Every `project/` artifact declares its metadata in YAML frontmatter
type: adr
date: 2026-10-28
updated: 2026-10-29
status: accepted
refs: [ADR-0161, ADR-0162]
applies_to: all repos with a `project` branch; `host` (`project/adrs/TEMPLATE.md`, `scripts/project-frontmatter.py`, `scripts/gen-adr-index.py`)
---

# ADR-0163: Every `project/` artifact declares its metadata in YAML frontmatter

## Decision

1. Every Markdown artifact under `project/` opens with a YAML header. Three
   keys are required everywhere: `title`, `type`, `date`.
2. `type` names the ADR-0162 taxonomy entry that owns the file — `adr`,
   `design`, `rfc`, `task`, `note`, `report`, `retro`, `plan`, `backlog`,
   `reference`, `principle`, `goal`, `readme`. `index` (generated) and
   `template` (skeleton) are the only exceptions; they need no `date`.
3. `date` is ISO-8601 (`YYYY-MM-DD`, or `YYYY-MM` when only the month is
   known): the day the artifact was created — the decision date for an ADR,
   the measurement date for a report or retro, the start of an investigation
   for a note. **The date never moves into the filename.** Filenames are stable
   labels: renaming breaks links, vendoring allowlists and citations.
4. ADRs carry `status` from the closed vocabulary `proposed | accepted |
   implemented | landed | superseded | absorbed | rejected | unrecorded`.
   A legacy record with no stated status is `unrecorded` — never guessed.
5. A supersession is structured, never prose: `status: superseded` plus
   `superseded_by: [ADR-NNNN]`; absorption uses `absorbed_into`. The record
   being replaced lists `supersedes: [ADR-NNNN]`. A `Superseded by` /
   `Supersedes` / `Absorbed into` sentence is promoted to the field and
   deleted; only rationale that does not name the relation stays in
   `status_note`.
6. **Status lives only in the header.** ADRs and RFCs use `status` (the enum in
   rule 4) plus an optional `status_note`; every other type (`plan`, `task`,
   `design`, `note`, `report`, `retro`) uses `status_note` verbatim, because it
   has no enum. A task's per-step statuses inside its own sections are progress,
   not the document's status.
7. `refs` is a list of ADR ids the document elaborates or relies on.
   `applies_to` is one prose line naming where a decision binds, when that is
   wider than the repo. `status_note` is the only other free-text field.
8. The body never carries status. Template bullets (`- Status:`, `- Date:`,
   `- Repos:`, `- Supersedes:`) are migrated into frontmatter and removed; a
   legacy `## Status` section (every occurrence) and a header `Status:` /
   `**Status**:` line are moved wholesale into `status_note` and deleted —
   except a supersession sentence, which goes to the relation fields. Compound
   metadata lines (`**Date**: … . **ADR**: …`) keep their non-date fields —
   migrate, do not rewrite.
9. `title` is the H1 without its identifier prefix (`ADR-NNNN: `,
   `RFC-NNNN: `, `Task: `); the H1 itself is unchanged, so existing anchors
   and links keep working.
10. `scripts/project-frontmatter.py` enforces the schema (`--write` migrates),
    `scripts/check-adrs.py` reads status and supersession from the header, and
    `scripts/gen-adr-index.py` renders the registry's date column from it.

## Why

The date rule sent dates into filenames (`notes/2026-08-24-tree-shaking.md`),
which makes every artifact rename on import — and the renames already broke
`vendor-docs.py`'s exclusion for a graphopt note that must not be published.
The metadata itself has four shapes: 28 ADRs use a `- Status:` / `**Status**:`
field, **159 carry status only as a `## Status` section**, and 2 say nothing
resolvable at all; 365 of 469 artifacts have no date outside git history. All of
it is prose, none of it queryable — and a status can disagree with the body two
sections below. One header makes the corpus queryable, the checks mechanical,
and the generated indexes honest.

## Not doing

- **Filename dates as the source of truth** — rename on import, no month
  precision, and the rename itself silently broke vendoring.
- **Parsing `## Status` prose as the status** — 159 styles, no enumeration;
  the section stays as prose, the header carries the decision.
- **A per-repo metadata registry** — drifts by construction (the hand-kept
  `docs-split.yaml` already lost ADR-0158 and ADR-0159).
- **YAML for everything** — eleven keys, closed; anything else is body prose.
- **Keeping a `## Status` section beside the header** — two homes, and the
  header is what tooling reads; the section is moved, not mirrored.
- **Keeping the `Superseded by ADR-N` sentence in `status_note`** — the
  relation is a link, not a sentence; prose duplicates drift from the field.

## Consequences

- 467 hand-written artifacts gain a header. Dates come from an explicit `Date:`
  line in 86, a filename prefix in 17, git's first-add commit in 357, and the
  file mtime in 7 (uncommitted files) — no date is invented, and the migration
  reports which source each one used. Two ADRs stay `unrecorded` until someone
  reads them.
- The 13 renames made for the filename rule are reverted; the graphopt
  vendoring exclusion (`notes/repo-split-plan.md`) works again.
- 200 `## Status` sections (including qualified headings such as `## Status of
  this ADR`) and 36 header status fields leave the bodies: their prose becomes
  `status_note` (217 files). No body carries status any more, so the header is
  the only place to look — or to change it.
- 20 status notes named a supersession. 16 were promoted to `superseded_by` /
  `supersedes` (or `absorbed_into`) and the sentence deleted; a note that was
  pure relation is gone, and only rationale survives. The remaining four are a
  partial supersession, a rewrite note, or a reference — no single field can
  represent them, so their prose stays.
- `adrs/TEMPLATE.md` grows a header, and `write-adr` points at it, so new
  records start compliant.
- Both generated indexes gain status + date columns, which is the first
  question anyone asks of the corpus.

## Enforcement

- `host/scripts/project-frontmatter.py` — required keys, type-per-path, date
  format, status vocabulary, supersession links, title/H1 drift.
- `host/scripts/check-adrs.py` — reads `status` / `superseded_by` from the header.
- `host/scripts/gen-adr-index.py`, `docs/scripts/gen-docs-index.py` — the
  rendered registry.

## Revisit if

- A second consumer needs a field (owner, milestone) — add it here rather than
  in one repo's header.
- More than 5% of ADRs are still `unrecorded` after a triage pass: the status
  rule is not being applied and the vocabulary needs to change.
