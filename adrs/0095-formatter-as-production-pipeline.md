---
title: Formatter as a Production Pipeline
type: adr
date: 2026-09-08
status: proposed
status_note: Proposed — formatter stages are data; `NodeEmitter` is the swap point.
---

# ADR-0095: Formatter as a Production Pipeline

## Decision

1. **Crate boundary**: the formatter keeps the abstraction/engine split —
   contract types and the formatting trait in one crate, the implementation in
   another.
2. **Pipeline and stage contracts**: parse → layout → emit, with stable
   contracts between stages; a stage can be replaced without touching its
   neighbours.
3. **Rule seam**: `FormattingRule` is replaceable and composable; formatting
   rules are data, not `kind ==` branches.
4. **Config seam**: formatting options are resolved once and passed as a
   config value, not read from globals.
5. **Engine seam**: `NodeEmitter` stays the swap point for a different layout
   strategy.
6. **Generic capability**: `platform/format.rs` is either wired to the real
   engine or retired explicitly — no dead second path.
7. **Language-as-data style**: no hardcoded kind strings; the formatter reads
   grammar roles and shapes.
8. **Performance**: parallel `--write`/`--check` across files and a
   stage-boundary cache keyed on content hash.

## Why

The formatter was a monolithic pass with hardcoded kinds and a second, dead
`FormatEngine` path, so a layout change touched the whole formatter. Stable
stage contracts and a rule seam make formatting data and the engine
swappable.

## Not doing
- **Keep one monolithic pass** — the coupling this removes.
- **Two format engines** — wire the capability or retire it.
- **Hardcoded kind strings** — roles and shapes come from grammar data.

## Consequences
- Stages and rules are replaceable; the emitter is the swap point.
- Parallel file formatting and stage caching become possible.
- One capability path, not two.

## Enforcement
- `engine-fmt/` — the formatter engine and stage contracts.
- `plugin-engine-fmt/` — the contract types (if separate).
- Formatter tests and the idempotence gate.

## Design
+Crate boundary, pipeline contracts, seams and performance notes:
+[`design/0095-formatter-as-production-pipeline.md`](../design/0095-formatter-as-production-pipeline.md).
