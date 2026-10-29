---
title: <the decision, stated as a fact — "X is Y", not "About X">
type: adr
date: YYYY-MM-DD
status: proposed          # proposed accepted implemented landed superseded absorbed rejected unrecorded
applies_to: <repos / crates this binds — omit if this repo only>
refs: []                  # [ADR-NNNN] records this one elaborates
---

# ADR-NNNN: <the decision, stated as a fact — "X is Y", not "About X">

## Decision

1. Up to ten numbered, normative, testable statements.
2. State what *is*, not what was tried, not what will be done next.
3. No code listings, no crate trees, no phase tables — those go to
   `project/design/` or `project/TODO.md`.

## Why

The mistake or failure this prevents, in at most eight lines. Name the real
cost: a bug class, a rewrite, a design that was rejected and re-proposed.
This is the memory of the decision — it must survive every later trim.

## Not doing

- `<alternative>` — rejected because <one line>.
- `<alternative we actually tried>` — cost <one line>.

One line each, no prose. This is what stops the same mistake being re-proposed.

## Consequences

- Up to five bullets: benefits *and* costs. If nothing got worse, nothing was
  decided.

## Enforcement

- `crates/path/src/file.rs`, `crates/x/tests/y.rs`, `…/wiring.rs` — where this
  is true in code.
- `none — proposed` is the only acceptable alternative, and only while the
  status is Proposed/Accepted.

## Revisit if

- A concrete trigger: a measurement, a dependency change, a scale threshold.
