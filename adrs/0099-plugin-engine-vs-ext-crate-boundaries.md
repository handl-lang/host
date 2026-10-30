---
title: Principled Boundaries Between `plugin-engine-*` and `plugin-ext-*` Crates
type: adr
date: 2026-09-08
status: proposed
status_note: Proposed.
---


# ADR-0099: Principled Boundaries Between `plugin-engine-*` and `plugin-ext-*` Crates

## Decision


Establish a strict crate-discipline rule:

1. `plugin-engine-<name>` crates are **generic, modular, composable engines**.
   - They operate on `handl_language::Language` data and are not tied to the canonical handl v0.1 grammar.
   - They contain no canonical rule sets, no embedded `handl-kit-data-canonical` data, and no `hnl` subcommand wiring.
   - They may expose operation markers, traits, and abstract request/response types (e.g. `Typecheck`, `TypecheckEngine`, `TypecheckRequest`).
   - A single engine crate may contain multiple internal modules/algorithms. The typechecker engine, for example, is one crate with modular passes: `nbe`, `refinement`, and `composite`.

2. `plugin-ext-<name>` crates are the **canonical handl bootstrap extension** for a tool.
   - They depend on the matching `plugin-engine-<name>` crate and on `handl-kit-data-canonical`.
   - They supply the canonical rules, language assembly, and the `hnl <name>` capability wiring.
   - They are the only crates that may embed canonical handl rule data or register a `hnl` subcommand.

3. Existing violations must be remediated:
   - Create `plugin-ext-typecheck` and move the canonical `check_module` entry point out of `plugin-engine-typecheck`. `plugin-engine-typecheck` should retain only the generic `Typecheck` operation abstraction and the modular engine implementations; `plugin-ext-typecheck` is the canonical `hnl typecheck` wrapper.
   - Merge `plugin-engine-typecheck-nbe` into `plugin-engine-typecheck` as an internal `nbe` module.
   - Convert the arrow-contract inference algorithm from `plugin-ext-refinement` into a `refinement` module inside `plugin-engine-typecheck`. Remove `plugin-ext-refinement` and the `hnlc` binary.
   - Add a `composite` (or `pipeline`) module in `plugin-engine-typecheck` that runs `nbe` then `refinement` on the appropriate fragments.

4. No `plugin-ext-*` crate may implement a generic algorithm. No `plugin-engine-*` crate may expose a `bin` target or a `hnl` capability.


## Why

The workspace contains `plugin-engine-*` and `plugin-ext-*` crates, but the distinction is not applied consistently. This creates confusion about whether a crate is a generic, data-driven engine or a handl-specific bootstrap tool.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- The `engine`/`ext` boundary becomes predictable and explainable.
- Generic engines can be reused by future language variants or test `Language`s without dragging in canonical handl data.
- The `hnl` CLI wiring is localized to `plugin-ext-*` crates, consistent with `host-cli`'s thin-dispatcher design.
- The typechecker becomes one modular, composable crate: `nbe` and `refinement` live as modules and can be run independently or composed.
- Refinement becomes a pluggable `TypecheckEngine` pass, consistent with ADR-0054, instead of a standalone binary.

## Enforcement

- `plugin-engine-*` — where this is true.
- `plugin-ext-*` — where this is true.
- `plugin-ext-typecheck` — where this is true.
- `plugin-engine-typecheck` — where this is true.
- `plugin-engine-typecheck-nbe` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0099-plugin-engine-vs-ext-crate-boundaries.md`](../design/0099-plugin-engine-vs-ext-crate-boundaries.md).
