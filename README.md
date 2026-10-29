---
title: '`project/` — what goes where'
type: readme
date: 2026-10-28
refs: [ADR-0161, ADR-0163]
---

# `project/` — what goes where

One screen. The rule is ADR-0162; this file is the map.

> **Published documentation is not here.** Explanations, tutorials, how-to
> guides and reference live in the org `docs` repo and are vendored into the
> docs site. `project/` holds the artifacts of *doing* the work.

| entry | answers | write when | lifetime |
|---|---|---|---|
| `adrs/` | *why* we decided, and what we rejected | a non-obvious decision, a ban, a dependency change | append-only; numbers never reused (ADR-0129 §3) |
| `design/` | *how* it works — mechanism, algorithms, state machines | the ADR's decision needs a page of detail | until the code is the spec |
| `rfcs/` | *when* we port a piece to Handl | a port is planned | until the port lands |
| `tasks/`, `TODO.md` | *who does what next* | work is scheduled | consumed |
| `BACKLOG.md` | *not now* — accepted but unscheduled | an idea survives triage without a date | reaped quarterly |
| `parked.yaml` | *retired* — deliberately not built, with an unpark trigger | a crate/idea is removed but the idea matters (ADR-0126 §3) | permanent |
| `notes/` | *what we found* — investigations, spikes, measurements | before you know the answer | promoted or deleted within one cycle |
| `reports/` | *what we measured* — audits, sweeps, migrations | after the measurement | dated (frontmatter), immutable |
| `retros/` | *what went wrong* — process post-mortems | after an incident or a bad estimate | dated (frontmatter), immutable |
| `references.md` | *what we depend on* — other repos' ADR ids + URLs | this repo relies on a foreign decision | maintained |
| `docs-split.yaml` | *where things live* — the assignment manifest | a path moves or a repo is split | maintained |

## Choosing in one question

```
Is it a decision?            → adrs/      (with Why + Not doing + Enforcement)
Does it explain mechanics?   → design/    (named after its ADR)
Is it a plan to port?        → rfcs/
Is it work with a date?      → tasks/ or TODO.md
Is it work without a date?   → BACKLOG.md
Is it deliberately dropped?  → parked.yaml
Is it a finding?             → notes/     (promote or delete)
Is it a measurement?         → reports/   (date in frontmatter)
Is it a process lesson?      → retros/    (date in frontmatter)
Is it for users?             → the org `docs` repo, not here
```

## Frontmatter — every artifact

Every Markdown file opens with YAML (ADR-0163). The date is **here**, never in
the filename:

```yaml
---
title: Tree-shaking in handl        # H1 without its `ADR-NNNN:` / `Task:` prefix
type: note                          # adr design rfc task note report retro plan …
date: 2026-08-24                    # created / decided / measured
status: proposed                    # ADRs and RFCs only, closed vocabulary
refs: [ADR-0161, ADR-0163]          # ADRs this document elaborates
---
```

| key | required | meaning |
|---|---|---|
| `title` | all | the H1, minus identifier prefix; the H1 itself is unchanged |
| `type` | all | the taxonomy entry above (`index`/`template` excepted) |
| `date` | all but `index`/`template` | ISO-8601 creation / decision / measurement date |
| `status` | ADRs, RFCs | ADR: `proposed accepted implemented landed superseded absorbed rejected unrecorded`; RFC: `draft accepted landed withdrawn` |
| `status_note` | no | the status text: the note behind the enum for ADRs/RFCs, the status itself (verbatim) for a plan, task, design, note, report or retro |
| `supersedes` / `superseded_by` / `absorbed_into` | no | `[ADR-NNNN]` links, required by the matching status |
| `refs` | no | `[ADR-NNNN]` this document elaborates |
| `applies_to` | no | one prose line when a decision binds beyond this repo |
| `updated` | no | ISO-8601 last substantive edit, living docs only |

Status lives **only** in the header: `status` + `status_note` for an ADR or RFC,
`status_note` alone for every other type. No `## Status` section, no `Status:`
line in the body — legacy ones were moved into `status_note` by the migration.
`unrecorded` is the honest status for a record that never stated one.

## Rules that are easy to get wrong

- **No `project/docs/`.** User-facing docs go to `docs/`, per repo at
  `docs/site/{overview,reference,how-to,explanation}`.
- **No copies across repos.** Cite `ADR-0150` + URL; never paste the file.
- **One `project/` per repo.** No `project/<area>/adrs/`.
- **Dated dirs are immutable.** A `reports/` page is never edited after the
  fact — write a second one. Its date is in the frontmatter, not the name.
- **Filenames are labels, not metadata.** No `2026-08-24-` prefixes; a date in
  a name renames the file and breaks links (ADR-0163).
- **One metadata home.** No `- Status:` / `- Date:` bullets, no `## Status`
  section, no `Status:` line in the body header — the header carries them.
- **A note is not a decision.** Rejecting a design produces an ADR, so the
  rejection is remembered.

Check: `python3 scripts/check-project-layout.py` (layout),
`python3 scripts/project-frontmatter.py` (metadata, ADR-0163) and
`python3 scripts/check-adrs.py` (ADR shape, ADR-0161).
