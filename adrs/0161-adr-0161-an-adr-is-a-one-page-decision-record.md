+++
id = "ADR-0161"
title = "ADR-0161: An ADR is a one-page decision record"
status = "open"
priority = "medium"
+++

---
title: An ADR is a one-page decision record
type: adr
date: 2026-10-28
updated: 2026-10-29
status: accepted
applies_to: '`host` (`project/adrs`, `scripts/check-adrs.py`), all repos with `project/adrs`'
---

# ADR-0161: An ADR is a one-page decision record

## Decision

1. An ADR is **at most ~8 KB and ten numbered rules**. Algorithms, tables,
   crate trees, code shapes and phase plans go to `project/design/`,
   `project/TODO.md` or `project/tasks/` — never into an ADR.
2. `## Why` is mandatory: the failure the decision prevents, in ≤8 lines.
   It is the memory of the mistake and is never trimmed.
3. `## Not doing` is mandatory: each rejected alternative on one line with its
   cost. This is what stops a rejected design being re-proposed.
4. `## Enforcement` is mandatory: the file or test where the decision is true
   in code, or `none — proposed`.
5. The status vocabulary is closed and lives in the frontmatter (ADR-0163):
   `proposed`, `accepted`, `implemented`, `landed`, `superseded`, `absorbed`,
   `rejected`, `unrecorded`. A successor is named by `superseded_by`; "partially
   superseded" is not a state — split the ADR instead.
6. A dead ADR keeps its **number** as a tombstone row (successor + one-line
   reason); its body is deleted. Numbers are never reused: code cites them.
7. Authority is single. When a decision replaces another, the replacement
   carries `Supersedes: ADR-N` in the same change, and there is exactly one
   design-of-record ADR per area.
8. `project/DECISIONS.md` is generated from `## Decision` sections — one line
   per rule with its ADR link. It is the recall surface; the ADR is the
   argument behind it.
9. `scripts/check-adrs.py` gates 1–7. Report-only for the legacy corpus;
   `--strict` for ADRs added or modified in a pull request.

## Why

The corpus is 189 ADRs / 1.9 MB and mostly unread: within `host`'s 112, 56
are over 8 KB, 36 carry more than ten rules, 101 record no rejected
alternative, and 109 do not name where they are enforced. The result is not a
shortage of records but a failure to *re-read* them — the same kernel was
specified five times with no supersession links (`0052` Implemented, `0103`
"the evaluator, not the calculus", `0106` "design of record"), and `0090`
(direct CLI dispatch) still contradicts `0149` (no built-in tools). Each
re-litigation costs a design session; each stale plan costs a reader an hour.

## Not doing

- **Leave ADRs as they are** — measured cost: 47% of bytes are unimplemented
  proposals, and two accepted ADRs contradict each other unnoticed.
- **Delete dead ADRs without tombstones** — breaks 300+ citations in code
  (`0044` ×100, `0077` ×67, `0094` ×56).
- **Move decisions to a wiki** — drifts from the code it describes; ADR-0129
  already keeps docs in the repo, on a `project` branch.
- **Keep the design detail in the ADR "just in case"** — it is already in
  `design/`, in the code, and in git history.

## Consequences

- The existing 53 oversized ADRs are grandfathered; they must satisfy this
  only when next edited, so no big-bang rewrite is needed.
- Losing the plan from ADRs pushes scheduling into `TODO.md`/`tasks/`, which
  must stay honest — a cost accepted.
- Compressing an implemented ADR no longer needs a supersession, because git
  keeps the history; only the decision text is normative.
- Reviewers get a mechanical check instead of a style debate.

## Enforcement

- `host/scripts/check-adrs.py` (size, decision count, mandatory sections,
  supersession link, duplicate numbers).
- `automation/skills/write-adr/SKILL.md` — the authoring path points here.
- `host/project/adrs/TEMPLATE.md` — the skeleton.

## Revisit if

- The strict gate blocks a legitimately large decision — then split the ADR
  rather than raising the budget.
