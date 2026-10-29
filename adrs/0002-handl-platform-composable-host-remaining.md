---
title: 'ADR-0002 (Remaining): Plugin Library Conversion and Module Abstraction'
type: adr
date: 2026-08-29
status: proposed
status_note: Proposed
---

# ADR-0002 (Remaining): Plugin Library Conversion and Module Abstraction

## Context

ADR-0002 v1 (Single Plugin Model for Host, Operations, Libraries, Targets, and Options) has been implemented and archived. The core architecture is complete: `Plugin`, `RuleAcceptor`, `Behavior<Op>`, and the removal of `Extension`/`CommandProvider`/`Library` legacy traits are done. The host is now a pure composer with no hard-coded stage wiring.

What remains is the conversion of library plugins to use the unified model.

## Decision

Complete the conversion of the remaining `plugin-lib-*` crates to work cleanly within the unified plugin model. Key questions to resolve:

1. _*Should plugin-lib-* crates use Behavior<Op> or is Module/Function the appropriate abstraction for host-exported functions?_*
   - Current state: ~20+ plugin-lib-* crates use `Module` trait and expose `Function` values for handl code to call
   - Option A: Convert to `Behavior<Op>` (one operation per library capability)
   - Option B: Keep `Module/Function` as the abstraction for "host functions that handl code calls" (distinct from rule contributions)
   - Option C: Hybrid — use `Module` as an adapter that contributes `Behavior<Op>` to the appropriate operations

2. **How should library dependencies be expressed?**
   - Can a plugin-lib-* declare that it requires or provides specific module capabilities?
   - Should this be done via `Plugin::depends_on()` trait or via a separate dependency manifest?

3. **What about library options?**
   - How do plugin-lib-* register options (e.g., compression level, timeout) if not via the legacy `Library::options()`?
   - Should the `OptionsPlugin` be extended to support library-scoped options?

## Consequences

- plugin-lib-* crates become clean participants in the unified plugin model, not legacy holdovers.
- Library behavior can be tested, mocked, and composed like all other plugin behavior.
- The platform/host contains no special-case logic for libraries.
- Kit manifests can be fully audited and composed without hidden library dependencies.

## Related work

- `project/adrs/archived/0002-handl-platform-composable-host.md` — original decision
- `project/adrs/archived/0002-handl-platform-composable-host-v1-complete.md` — v1 completion (Behavior<Op>, core architecture)
- ADR-0040 (VM is an operation, not a subsystem) — ExecControl exception for eval-specific concerns
- Crates: ~20+ plugin-lib-* (http-server, json, collections, intl, etc.)
