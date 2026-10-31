+++
id = "ADR-0047"
title = "ADR-0047: Meta Functions as `plugin-lib-*` Crates"
status = "open"
priority = "medium"
+++

---
title: Meta Functions as `plugin-lib-*` Crates
type: adr
date: 2026-09-02
status: accepted
status_note: Accepted — meta functions are five leaf crates; `run` is composition.
---

# ADR-0047: Meta Functions as `plugin-lib-*` Crates

## Decision

1. **Five new leaf crates, one operation each** — `eval`, `lex`, `parse`,
   `compile`, `run-compiled` — instead of traits on the runtime.
2. **`handl-runtime` becomes the abstraction + `Runtime` impl**; engines
   register by name.
3. **`compile` / `run_compiled` are leaf-able** via `CompiledHandle` and
   `CompiledArtifact`.
4. **`run` is dropped** — it is composition of the other operations, not a
   primitive.
5. **`self` stays an ambient global**, handed to the program.
6. **Scope types re-export from `plugin-lib-eval`**, so there is one type per
   concept.

## Why

Meta functions were traits attached to the runtime, so every engine and every
lib had to know the runtime type. Leaf crates with one operation each follow
the plugin model: a host registers operations by name and composes them.

## Not doing
- **Keep `run` as a primitive** — it is `eval` + module composition.
- **Keep meta traits on the runtime** — they couple every lib to the runtime.
- **Duplicate scope types per lib** — re-export from `plugin-lib-eval`.

## Consequences
- Five small crates replace a trait bundle; registration is by name.
- Compiled programs travel as handles/artifacts, not closures.
- Composition lives in handl, not in the runtime.

## Enforcement
- `crates/plugin-lib-lang-{eval,lex,parse,compile,run-compiled}/`.
- `handl-runtime/` — the abstraction and name registry.
- `plugin-lib-eval/` — the scope types.

## Design
+The five crates, registration, handles/artifacts and the scope re-exports:
+[`design/0047-meta-functions-as-plugin-lib-crates.md`](../design/0047-meta-functions-as-plugin-lib-crates.md).
