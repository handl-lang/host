+++
id = "ADR-0046"
title = "ADR-0046: Capability Discipline at the Language Level"
status = "open"
priority = "medium"
+++

---
title: Capability Discipline at the Language Level
type: adr
date: 2026-08-30
status: proposed
status_note: Accepted — capabilities are declared statically and enforced at the eval boundary.
---

# ADR-0046: Capability Discipline at the Language Level

## Decision

1. **`CapabilitySet` summarizes a `Language` statically** — host function
   names, whether it has `vau`, and whether it can `eval` or `compile`.
2. **It is computed by scanning eval schemes**: `Foreign` adds a name, `Vau`
   implies eval, and `eval`/`run`/`compile` names set the corresponding flags.
3. **`CapabilityPolicy` declares what a context allows** — host fns, `vau`,
   `eval`, `compile`.
4. **A static check at the `eval`/`run`/`compile` boundary** denies before
   dispatch; it reads `Language` data, not the running program.
5. **`LanguageRegistry` carries policies**, attached at registration time by
   the host that builds it.
6. **Sub-runtimes inherit a narrowed policy** — a sandboxed language cannot
   escalate by evaluating a more privileged one.
7. **`capabilities(handle)` is a read-only host function** for handl programs,
   and `CapabilityDenied` surfaces a denial to the program.

## Why

Capabilities were enforced only at runtime, so a language could smuggle in
`vau`/`eval`/`compile` or an unlisted host function and no one could inspect
the requirement. A static set plus a policy check at the boundary makes the
sandbox reviewable before execution.

## Not doing
- **Runtime instrumentation** — the check is static and once per boundary.
- **Per-call policy changes** — policies attach at registration.
- **Capability escalation via sub-runtimes** — inherited policies are narrowed.

## Consequences
- Sandboxing is inspectable and cheap; capability changes are data changes.
- A host must declare a policy per registered language.
- `capabilities()` lets handl programs reject unsafe languages.

## Enforcement
- `crates/handl-language/` — `CapabilitySet`, `CapabilityPolicy`.
- `handl-runtime/` — registry and boundary check.
- `capabilities(handle)` host function; denial tests.

## Design
+Struct shapes, computation rules, boundary check and inheritance:
+[`design/0046-capability-discipline-at-the-language-level.md`](../design/0046-capability-discipline-at-the-language-level.md).
