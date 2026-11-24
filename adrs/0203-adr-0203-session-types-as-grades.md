+++
id = "ADR-0203"
title = "ADR-0203: Session Types as Grades"
status = "open"
priority = "med"
+++

---
title: Session Types as Grades
type: adr
date: 2026-09-09
status: proposed
status_note: 'Accepted — sessions are grades: `!T`, `?T`, `End`, with duality as a law.'
---

# ADR-0203: Session Types as Grades

> Imported from `kit-data-prelude/project` — originally `ADR-0102` (global registry repo `lang`); renumbered `ADR-0203` in the host merge.

## Decision

1. **Introduce `grade Session`** with `!T` (send), `?T` (receive) and `End`;
   a protocol is a grade expression built from them.
2. **Duality `dual : Session -> Session`** flips every action (`!T ↔ ?T`,
   `End ↔ End`) and is an involution: `dual(dual(S)) = S`.
3. **Sequential composition is grade multiplication** — `!Int ; ?Bool ; End`
   reuses the existing composition syntax.
4. **Channel operations are graded arrows** whose grade is a `Session` object.
5. **Sessions are checked like any grade** — no separate session type system.

## Why

Session protocols were a separate type discipline, so a channel could not be
composed with the rest of the grade system. Making a session a grade reuses
duality, composition and checking, and keeps protocols data.

## Not doing
- **A separate session type system** — grades already compose.
- **Hardcode send/receive** — they are grade constructors.
- **Non-involutive duality** — `dual(dual(S)) = S` is a law.

## Consequences
- Protocols are grade expressions; channel ops are graded arrows.
- Duality is a grade operation and a law.
- No new checker path.

## Enforcement
- `crates/handl-kit-data-prelude/` — the `Session` grade family.
- Grade laws for duality; channel op grades.
- Protocol tests.

## Design
+Constructors, duality, composition and channel operations:
+[`design/0203-session-types-grades.md`](../design/0203-session-types-grades.md).
