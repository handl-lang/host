# ADR-0085: Bootstrap Tooling Strategy — Rust Tooling on the Generic Substrate

## Status

Implemented. Amends [ADR-0014](../0014-formatter.md),
[ADR-0015](../0015-linter.md), and
[ADR-0029](../0029-cli-as-capability-orchestrator.md): their generic,
plugin-driven designs are **retained in Rust**; what changes is that the
canonical tooling is now concrete, arrow-surface-focused, and shipped by
improving the existing plugin crates. Depends on
[ADR-0074 — Arrow Surface Syntax v0.1](./0074-arrow-syntax-v0_1-surface-redesign.md)
and [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md).
Amended by decision 2026-09: the Rust substrate (plugin system,
`plugin-ext-*`, `plugin-engine-*`) is **kept and improved, not deleted** —
the move to handl is a future port, not a rewrite.

## Context

The self-hosting goal is for the CLI, formatter, linter, typechecker, test
runner, and eventually the LSP server to be written in handl itself. That
port can only happen once handl is stable — and to _get_ stable, the
stdlib and real programs must be written in handl first, which requires
production-grade tooling today.

The good news from an inventory of the existing platform: a substantial,
already-generic Rust substrate exists and works — the plugin system
(`platform` crate: `Capability`/`Plugin`/`Kit`/`Host`), the capability
extensions (`plugin-ext-*`), and the generic engines (`plugin-engine-lex`,
`plugin-engine-parse`, `plugin-engine-parse-pratt`,
`plugin-engine-eval*`, `plugin-engine-typecheck*`). Much of the work is
done; it just was never focused into a concrete toolchain for the arrow
surface.

## Decision

Build **the canonical tooling by improving the existing generic `plugin-*`
crates** — they already _are_ the tooling; no parallel `tooling-*` crates
are created:

| Concern            | Crate(s) to improve                                                               |
| ------------------ | --------------------------------------------------------------------------------- |
| Formatter          | `plugin-ext-fmt` (canonical style hardcoded in its rules)                         |
| Linter             | `plugin-ext-lint` (canonical rule set over the generic registry)                  |
| Typechecker        | `plugin-engine-typecheck`, `plugin-typecheck-core`, `plugin-typecheck-nbe`        |
| Test runner        | `plugin-lib-test` (plus its capability wiring)                                    |
| CLI                | `host-cli` + `plugin-ext-cli` (thin dispatch over the above)                      |
| Shared diagnostics | **new `plugin-ext-diag`** (see ADR-0091) — one home for `Diagnostic`/`Span`/`Fix` |

Rules of engagement:

1. **The substrate stays and is the implementation.** The plugin system,
   `plugin-ext-*`, and `plugin-engine-*` crates are not replaced — they are
   worked on, improved, and extended in place. Tool features land in the
   existing plugin crates through the same `Capability` contracts.
2. **Best practices in Rust, now.** Composability, extensibility,
   modularity, testability, and replaceability are requirements — the same
   properties the future handl tools will have. Improving in place is not a
   license for coupled monoliths.
3. **Canonical style/rules are hardcoded; the machinery is generic.** What
   may be coupled to the v0.1 surface (ADR-0074) is the _rule set_ and
   _canonical preferences_. The engines, pipelines, and extension points
   underneath stay generic (per-language capabilities per ADR-0041), so
   other handl languages still benefit.
4. **Minimal decency, not prototyping quality.** Small module boundaries
   inside each plugin crate, **100% test coverage** enforced in CI, and one
   shared diagnostics type family in `plugin-ext-diag` (ADR-0091) so
   fmt/lint/typecheck/test/CLI speak the same diagnostics language.
5. **The CLI stays thin.** `host-cli` gains/keeps subcommands that dispatch
   through the existing capability contracts; no _new_ plugin protocol is
   invented. The full generic capability-registry CLI of ADR-0029 remains
   the design, exercised for real by this toolchain.
6. **Port to handl in the future, improve in place until then.** When handl
   is stable, each tool moves to handl per its RFC (`project/rfcs/`) —
   a port of already-generic code, not a rewrite. The Rust substrate
   remains the bootstrap and fallback implementation afterwards.

## Consequences

- We can finally write the best handl code in handl itself: stdlib, tests,
  and examples get fmt/lint/typecheck/test from day one.
- No mass deletion and no duplicate crates: the existing `plugin-*` tooling
  crates keep their tests, their modularity, and their users, and are
  improved in place. Diagnostics are consolidated in `plugin-ext-diag`
  (ADR-0091) instead of accumulating per-crate variants.
- Style and rule changes are Rust edits until the handl port — acceptable
  while only the v0.1 surface exists, and cheap because rules are isolated
  modules.
- The future handl port is lower-risk: the RFCs port _generic, tested_
  Rust designs whose contracts are already exercised.

## Status

Implemented (merged to main).
