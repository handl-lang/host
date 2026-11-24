---
title: Task 0128 — `lang/` extraction (ADR-0128 Phase 11)
type: task
date: 2026-10-15
---

> Imported from `kit-data-prelude/project/tasks/0128-extraction.md` during the host ledger merge.

# Task 0128 — `lang/` extraction (ADR-0128 Phase 11)

Expand ADR-0128 Phase 11 for this repo. The docs are already on
this `project` branch; the code lands with the extraction.

## Docs owned here

- `adrs/0206-language-data-assembly.md` — Language data assembly in the lang repo
- split parent `0044-language-as-data-and-runtime-split.md` (canonical half stays in `host`)

## Extraction

- [ ] Follow ADR-0128 Phase 11: create the repo's workspace,
      move the crates/packages named there, wire the git dep + `[patch]`.
- [ ] Keep `main` a clean build tree (`/project/` ignored).
- [ ] Flip the split children from `Proposed` to `Accepted` when the
      code actually moves.
