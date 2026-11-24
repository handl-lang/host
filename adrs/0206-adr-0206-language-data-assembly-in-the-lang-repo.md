+++
id = "ADR-0206"
title = "ADR-0206: Language data assembly in the `lang` repo"
status = "open"
priority = "med"
+++

---
title: Language data assembly in the `lang` repo
type: adr
date: 2026-10-15
status: proposed
status_note: Proposed — extracted in ADR-0128 Phase 11.
---

# ADR-0206: Language data assembly in the `lang` repo

> Imported from `kit-data-prelude/project` — originally `ADR-0130` (global registry repo `lang`); renumbered `ADR-0206` in the host merge.

## Context

[ADR-0044](../0044-language-as-data-and-runtime-split.md) made the language
*data* and its *runtime* separately addressable. [ADR-0128](../0128-target-repos.md)
Phase 11 extracts the data half out of `platform` so a consumer can take the
canonical language without the engine workspace.

## Decision

`lang` owns **the canonical language as data**:

- `crates/handl-kit-data-canonical` — the fragment merger and the canonical
  `data/*.yaml` (`ast`, `eval`, `types`, `grammar`, `transforms`,
  `value_types`, `representations`) plus `data/kits/*.yaml`; and
- the `.hnl` packages it pins (`query`, `command`, `event`, …).

It exposes the assembled `Language`. Platform engines are consumed as git
dependencies; no engine, runtime, or host code lives here.

## Consequences

- A language change is a YAML edit in this repo, not an engine edit (the
  ADR-0044/ADR-0058 rule, now enforced by a repo boundary).
- `plugin-tool-test`, `plugin-ext-lint` and `plugin-ext-typecheck` take the
  `Language` by injection rather than linking this crate (the ADR-0128 §0.8
  seam) — otherwise the dependency cycles.
- The parent ADR-0044 stays in `host`: it also decides the runtime split,
  which is platform's.
