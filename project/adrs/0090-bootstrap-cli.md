# ADR-0090: Bootstrap CLI (`host-cli`)

## Status

Proposed. Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md)). Deliberately narrows
[ADR-0029 — CLI as Capability Orchestrator](../0029-cli-as-capability-orchestrator.md)
for the bootstrap phase; the full plugin-driven, generic CLI returns as
[RFC-0007](../rfcs/0007-cli-in-handl.md).

## Context

The tooling improvements (ADR-0086/0087/0088/0089) need a single entry
point, and the stdlib needs run/eval access. `host-cli` (with
`plugin-ext-cli`) already provides the `hnl` binary and capability wiring;
what it needs is the canonical subcommand surface over the improved plugin
crates. A *new* sophisticated plugin-loading protocol is premature while
there are exactly these tools and one language surface.

## Decision

The `hnl` binary stays in `crates/host-cli` (with `crates/plugin-ext-cli`),
dispatching directly to the improved tooling crates and engines:

```text
host-cli (hnl)
├── plugin-ext-fmt
├── plugin-ext-lint
├── plugin-engine-typecheck
├── plugin-lib-test
├── plugin-engine-lex / -parse / -eval / -typecheck   (run + engines)
├── handl-language, handl-value, handl-runtime        (core surfaces)
└── platform                                          (contracts only)
```

- **Command surface (v0.1):**

  ```bash
  hnl fmt   [--check] [--write] [paths...]
  hnl lint  [--fix] [--fail-on-warn] [paths...]
  hnl check [paths...]
  hnl test  [--filter <pat>] [--list] [--json] [paths...]
  hnl run   <file.handl> [-- <args...>]
  hnl eval  '<expression>'
  ```

- **No *new* plugin system.** No capability registry, no dynamic loading,
  no process-protocol plugins are invented here — subcommands dispatch to
  the tool crates, which expose themselves through the existing
  `plugin-ext-*` capability contracts (ADR-0085 rule 5). This exercises
  ADR-0029's design for real; RFC-0007 later re-hosts the same contracts
  in handl.
- **Coupling policy:** the CLI is allowed to know about every tool crate,
  but tool crates never depend on the CLI or on each other (the linter
  consumes `plugin-ext-fmt`'s public API only, via a workspace-internal path —
  recorded as the single sanctioned tool-to-tool dependency).
- **Exit codes:** `0` success; `1` check/lint/test failure; `2` usage or
  parse error. Uniform, documented, CI-friendly.
- **Diagnostics:** all subcommands print through the shared renderer in
  `plugin-ext-diag` (ADR-0091): spans →
  `file:line:col: severity [rule]: message`, identical across tools.
- **Testing:** 100% coverage of argument parsing, dispatch, exit-code
  mapping, and output shaping via integration tests running the real
  binary; e2e fixtures under `tests/fixtures/`.

Explicitly deferred: package management, plugin install, target selection,
config discovery, `handl lang` namespaces (RFC-0007).

## Consequences

- One `hnl` binary covers the entire bootstrap workflow; CI (`fmt --check`,
  `lint`, `check`, `test`) is trivially scriptable.
- Replacing the CLI at RFC-0007 time is cheap because all logic lives in
  the tool crates; the binary is a thin dispatch shell (~no business logic).
- The existing `host-cli` gains the `hnl` subcommands by consuming the new
  tooling crates through the capability contracts; replacing its embedded
  pipeline logic is a refactor, not a deletion (ADR-0085 rule 1).

## Status

Proposed
