+++
id = "ADR-0041"
title = "ADR-0041: Languages as First-Class Values"
status = "open"
priority = "medium"
+++

---
title: Languages as First-Class Values
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — `Language` and `eval` are handl values/functions.
---

# ADR-0041: Languages as First-Class Values

## Decision

1. **`Language` is a handl-level type** — a language definition is a value the
   program can hold, pass and inspect.
2. **`eval` is a handl function** — evaluating an AST is a function call, not a
   privileged host operation.
3. **`self` is the running language**, handed to the program, so a program can
   evaluate in its own language.
4. **Language combinators are handl functions** — composing languages is
   ordinary composition.
5. **The bootstrap is a compilation step, not interpretation** — the initial
   language is compiled, then evaluated as data.
6. **Structured by default; `vau` is a typed escape** for the cases structure
   cannot express.
7. **Capability discipline at the language level** — a language declares what
   it may do; the host enforces it.

## Why

`Language` and `eval` lived in Rust, so a program could not inspect or extend
its own language and `self` was a host notion. Values and functions make
self-hosting ordinary handl, and capability discipline keeps it safe.

## Not doing
- **Keep `Language`/`eval` host-only** — self-hosting would stay impossible.
- **Interpret the bootstrap** — it is compiled once, then data.
- **Untyped `vau`** — the escape is typed so capability discipline holds.

## Consequences
- Languages are data; combinators are handl functions.
- `self` becomes an ambient value, not a host flag.
- `vau` is the only escape hatch and is typed.

## Enforcement
- `kernel/` — `Language` value, `eval` function, `self`.
- `crates/handl-kit-data-canonical/` — the language data.
- Self-hosting tests (`handl-defining-handl` / dogfood gate).

## Design
+The seven decisions in full, the bootstrap, `vau` and the capability model:
+[`design/0041-languages-as-first-class-values.md`](../design/0041-languages-as-first-class-values.md).
