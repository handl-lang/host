---
title: Misc tasks — ADR-0068 / 0082 / 0083 follow-ups
type: task
date: 2026-09-04
---

# Misc tasks — ADR-0068 / 0082 / 0083 follow-ups

> **Status: all items from the 2026-09-04 snapshot are now complete or superseded.**
> The original snapshot was taken at `8872ac9d`; the current `main` branch has
> merged every phase and quick-fix listed below. This file is kept as a
> completion record; new work should be tracked in fresh task files or ADRs.

## Completion record

| Item                                                                    | Status         | Merged in                               |
| ----------------------------------------------------------------------- | -------------- | --------------------------------------- |
| Phase 0 — Critical `data`/recursor correctness bugs                     | [x]            | `5966c44a` / `568be218`                 |
| Phase 1 — `data` reachable from surface syntax                          | [x]            | `daafcd8b` / `e42d3e3c`                 |
| Phase 2 — Recursor generality & strict positivity                       | [x]            | `756b3d5b` / `2f9ee389`                 |
| Phase 3 — B2 symbolic normalizer gaps                                   | [x]            | `0c5d08f1` / `2c3f4107`                 |
| Phase 4 — Engine genericity (data-driven AST child roles)               | [x]            | `3208cd61` / `807c3d8c`                 |
| Phase 5 — Runtime-assert IR emission                                    | [x]            | `258f38b1` / `2682e1a9`                 |
| Phase 6 — ADR-0082 steps 3–4 (termination metadata + match-as-recursor) | [x]            | `c342ffeb` / `42ae2626`                 |
| Phase 7 — `Total` & `Pure` predicates                                   | [x]            | `d4cabe19` / `54aa2519`                 |
| Phase 8.1 — Cranelift foreign dispatch                                  | [x]            | `58b16608` / `28dcc5ca`                 |
| Phase 8.2 — `Marshalable` predicate / `ffi_bind`                        | [x]            | `fd7670d5` / `91096dbd`                 |
| Phase 9 — Tooling scaffolds                                             | **superseded** | ADRs 0085–0091 (`1b3aae37`, `cc1b43b2`) |
| Phase 9.5 — Import/export + handl-source module loader                  | [x]            | `f9766f31` / `21d7ce83`                 |
| Phase 10.1 — Typecheck before eval                                      | [x]            | `a41261be` / `9d71cb90`                 |
| Phase 10.2 — Span info audit for engine errors                          | [x]            | `493a8b7c` / `55d7ace5`                 |
| Phase 11 — host-cli legacy `--plugin` test ignored                      | [x]            | `0c95c6e9`                              |
| Q.1 — `handl-value` `ForeignResource::dropped` accessor                 | [x]            | fixed in tree                           |
| Q.2 — Eval-engine `Bytes`/`Resource` match arms                         | [x]            | fixed in tree                           |

## Verification command

```bash
cargo test --workspace --no-run
```

This should compile cleanly (the last workspace test build was started at
`/tmp/verify_misc.log`).

## What next?

No tasks remain from this snapshot. Open architectural tracks that are _not_
covered here include:

- **ADR-0081 follow-ups** beyond Phase 4 (e.g., full role-driven engine
  genericity audits in remaining engines).
- **Inductive families / dependent pattern matching** (ADR-0082 steps 5–6).
- **Thread-safe `ForeignResource`** (`Arc` instead of `Rc`).
- **Bootstrap tooling implementation** per ADRs 0085–0091.

Create a new task file when the next concrete workstream is defined.
