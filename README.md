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
| `reports/` | *what we measured* — audits, sweeps, migrations | after the measurement | dated, immutable |
| `retros/` | *what went wrong* — process post-mortems | after an incident or a bad estimate | dated, immutable |
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
Is it a measurement?         → reports/   (dated)
Is it a process lesson?      → retros/    (dated)
Is it for users?             → the org `docs` repo, not here
```

## Rules that are easy to get wrong

- **No `project/docs/`.** User-facing docs go to `docs/`, per repo at
  `docs/site/{overview,reference,how-to,explanation}`.
- **No copies across repos.** Cite `ADR-0150` + URL; never paste the file.
- **One `project/` per repo.** No `project/<area>/adrs/`.
- **Dated dirs are immutable.** A `reports/` page is never edited after the
  fact — write a second one.
- **A note is not a decision.** Rejecting a design produces an ADR, so the
  rejection is remembered.

Check: `python3 scripts/check-project-layout.py` (layout) and
`python3 scripts/check-adrs.py` (ADR shape, ADR-0161).
