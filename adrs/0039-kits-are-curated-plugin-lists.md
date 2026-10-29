---
title: Kits Are Curated Plugin Lists, Not Implementations
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Supersedes the "zero-cost static dispatch" clause of ADR-0034.

  Proposed. Implementation deferred until this ADR is accepted.
---

# ADR-0039: Kits Are Curated Plugin Lists, Not Implementations

## Context

The handl platform has a host-plugin system. The language, runtimes, stdlib,
targets, and extensions are all composed of plugins. Kits are curated sets of
plugins and other kits. Hosts are agnostic binaries that load a kit and run it.

That is the intended architecture. It is not what the seven `kit-*` crates are
today.

### What the `kit-*` crates are today

**`kit-canonical`** is 2,629 lines holding the entire handl-language
implementation: the unified token kind, the unified AST, the lexer, 16 AST
converters, a parser behavior, grammar/manifest wiring, and the parse pipeline.
The curated plugin list (`CanonicalKit::contribute`) is ~25 lines at the end.
The kit _is_ the implementation, not a curation of it.

**`kit-hello`** is the target shape: 66 lines, `contribute` registers
`CanonicalKit` + `HelloLibraryPlugin`. Pure curation.

**`kit-cli`** is half curation (`CliKit`), half VM host logic (`CliHost` — a
`Host` impl wiring ~20 modules, resource limits, `new_context`).

**`kit-config`** and **`kit-server`** are pure VM host logic (`SandboxHost`,
`ServerHost`) with no `Plugin`, no manifest, no curation. They should be kits
curating the sandbox and server plugin sets; instead they are `Host` structs.

**`kit-type`** and **`kit-type-schema-json`** are libraries (a type-level VM
and a JSON-Schema bridge), not kits. They have no `Plugin`, no manifest, no
curation. `kit-type` was expected to be a kit that curates type plugins; it is
an algorithm.

### Why this happened

ADR-0034 chose static composition ("zero-cost static dispatch, compile-time
trait safety"). That decision **made curation impossible**. A curated list of
plugins is meaningless when the composition result is a compile-time enum —
you cannot curate a different plugin set and get a different unified kind
without recompiling. The static `CanonicalKind` enum forced all token/AST/
grammar logic into one crate. That crate became `kit-canonical`, and the kit
became the monolith.

The tiny plugins were already designed for composition — `plugin-grammar-call`
is generic over `K: CallToken`. But static composition required a single
concrete type (`CanonicalKind`) with 19 trait impls bridging it to every
capability. That bridge layer is the monolith.

### What this means for hosts

Because kits embed implementation, hosts are coupled to specific kits. Adding a
new plugin or building a new kit requires touching host code. Hosts are not
agnostic. This blocks the core goal: a host loads any kit and runs it, and
adding plugins never touches a host.

## Decision

### Architecture

**Plugins** offer language features, extensions, stdlib, runtime, targets —
everything. A plugin is the unit of composition.

**Kits** are curated sets of plugins and other kits. A kit defines what runs
where: which language features, which stdlib, which runtime, which target.
Examples:

- `kit-canonical` — the handl language (lexer, parser, grammar, AST, eval, …).
- `kit-config` — handl as a configuration language (pure stdlib, no OS, no
  network, resource-limited).
- `kit-cli` — handl for building CLI tools (OS stdlib, utils, collections).
- `kit-server` — handl for long-running servers (extends `kit-config` with OS,
  HTTP, rand, test, otlp).
- `kit-type` — handl's type system (curates typecheck plugins).
- `kit-hello` — demo (extends `kit-canonical` with the hello library).

Kits compose: `kit-server` extends `kit-config` extends `kit-canonical`. A kit
is a manifest plus thin registration glue — nothing more.

**Hosts** are agnostic binaries. A host loads a kit and runs it. Adding a
plugin, building a new kit, or swapping a runtime never touches host code.
Hosts are completely decoupled from what the kit curates.

### Composition is dynamic

Supersedes ADR-0034's "zero-cost static dispatch" clause. The composition
result is built at kit-load time from the curated plugins, not baked into a
compile-time enum. This is what makes curation real: a different kit curates a
different plugin set and gets a different language, without recompiling a
unified type.

### Principle: plugins offer capabilities, kits only curate

Capabilities (lex, parse, eval, fmt, lint, typecheck, build, compile, stdlib
modules, …) are offered by plugins. A kit includes whichever plugins it wants;
it does not implement them. Kits are pure curation.

### Naming principle

Plugin names are kit-agnostic. "canonical" is the name of one kit, not a
property of the plugins. A non-handl kit composes its own
`plugin-token-<lang>` / `plugin-ast-<lang>` instead. No `plugin-*-canonical`
names. Some plugins are language-specific (`plugin-ast-handl-value`); that is
fine — they are reusable across handl-flavored kits, just not across
non-handl languages.

### Phase 1 — `kit-canonical` becomes a thin kit

The 2,629-line `lib.rs` dissolves. The curated list stays; everything else
moves to plugins or the framework:

| Logic                                                        | New home                                                                                                                                                                                      |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unified token kind, factory, 19 capability impls             | Per-plugin registry entries in `plugin-token-*`; the framework composes them dynamically at kit-load time.                                                                                    |
| Unified AST node + converters                                | Per-shape plugins (`plugin-ast-stmt-import`, `plugin-ast-stmt-export`, `plugin-ast-stmt-module`, `plugin-ast-handl-value`, existing `plugin-ast-*`); the framework composes them dynamically. |
| Lexer + config                                               | `plugin-ext-lexer` — a generic lexer that assembles rules from registered `plugin-lexer-*` plugins.                                                                                           |
| `TypeAnnotRule`                                              | `plugin-grammar-type-annot` — one tiny grammar-rule plugin.                                                                                                                                   |
| Grammar/manifest wiring, rule registry                       | Self-registration in `plugin-ext-parser` / `handl-platform`; each `plugin-grammar-*` registers its rule. No central match statement.                                                          |
| Parse pipeline (`lex`/`parse`/`parse_module`/`grammar_info`) | `plugin-ext-parser` — offers the parse capability; a kit includes it if it wants parsing.                                                                                                     |
| `Diagnostic` / `Diagnostics`                                 | `plugin-diag-core` — generic.                                                                                                                                                                 |
| `Stmt` / `StmtKind` / `ModuleOutput`                         | Dissolved into per-statement `plugin-ast-stmt-*` crates.                                                                                                                                      |
| `CanonicalKit::contribute` (the curated list)                | **Stays in `kit-canonical`.**                                                                                                                                                                 |

`kit-canonical` shrinks to ~the size of `kit-hello`.

### Phase 2 — Kits and hosts are separated

**Kits become thin curated lists:**

- `kit-config` — curates the pure/sandbox plugin set (no OS, no HTTP, no rand).
- `kit-server` — curates the server plugin set (OS + HTTP + rand + test +
  otlp); extends `kit-config`.
- `kit-cli` — curates the CLI plugin set; `CliKit` stays.

**`Host` trait impls are deleted — not moved to new binaries.**

`SandboxHost`, `ServerHost`, and `CliHost` are not binaries — they are `Host`
trait impls that wire modules into a `ModuleHost`. That wiring is exactly what
the kit curates. With dynamic composition, a generic host builder reads the
kit's curated `plugin-lib-*` plugins, registers them into a `ModuleHost`,
applies resource-limit policy, and runs. No per-use-case `Host` struct needed.

- `SandboxHost` — **deleted.** Its module list becomes `kit-config`'s curated
  plugins. Resource limits become host configuration or a policy plugin.
- `ServerHost` — **deleted.** Its module list becomes `kit-server`'s curated
  plugins.
- `CliHost` — **deleted.** Its module list becomes `kit-cli`'s curated plugins.

**Hosts remain generic binaries:**

- `host-cli` (exists) — loads any kit, runs it.
- `host-lsp` (exists) — loads any kit, serves LSP.
- No `host-sandbox`, no `host-server` — those use cases are `kit-config` and
  `kit-server` loaded by an existing generic host.

A host loads a kit, dynamically assembles the runtime from the kit's curated
plugins, and runs. The kit is the source of truth for what's included. Adding
a plugin or building a new kit never touches a host.

### Phase 3 — `kit-type` becomes a real kit

- NBE type-VM algorithm → `plugin-typecheck-nbe`.
- JSON-Schema bridge → `plugin-typecheck-schema-json` (or fold into
  `plugin-lib-type`).
- `kit-type` becomes a thin kit curating `plugin-typecheck-nbe` +
  `plugin-typecheck-core` + `plugin-lib-type` + the schema bridge.
- `handl-platform` and `plugin-lib-type` depend on the new plugin locations.

### Phase 4 — Update consumers and workspace

- Update imports in `host-cli`, `host-tooling`, `host-lsp`, `plugin-ext-kit`,
  `plugin-lsp-*`, `plugin-typecheck-core`, `plugin-lib-type`, `platform`.
- Update workspace `Cargo.toml`.
- Move benches and tests.
- Verify with `cargo test --workspace`.

## Consequences

### Positive

- Every `kit-*` crate is a thin, auditable curated list — like `kit-hello`.
- All logic lives in reusable, composable, replaceable plugins. A new kit
  composes a different plugin set and gets a different language, without
  forking or recompiling.
- Hosts are agnostic. Adding a plugin or building a new kit never touches a
  host. A host loads any kit and runs it.
- `kit-type` becomes what its name promises: a kit curating type plugins.
- `kit-config` and `kit-server` become kits curating the sandbox and server
  plugin sets; `kit-server` extends `kit-config`. No `host-sandbox` or
  `host-server` binaries — those use cases are kits loaded by a generic host.
- The monolith that grew back into `kit-canonical` after ADR-0034 is dissolved.
  ADR-0001's vision ("a language only includes the node shapes it loads") is
  achieved through dynamic composition.

### Negative

- Crate count increases.
- Wide blast radius: many consumers need import updates and migration from
  static unified types to dynamic composition.
- Going dynamic supersedes ADR-0034's static-dispatch clause. The cost is
  indirect dispatch on token/AST inspection, which is not the hot path
  (parse/eval are). The framework must be redesigned for object safety (see
  Implementation notes).
- The framework must gain runtime self-registration registries. This is real
  framework work, not just code movement.
- Benches and tests need to move and may need adjustment.

## Implementation notes

These are engineering problems to solve in service of the architecture, not
reasons to question the decision. They are called out here for transparency;
detailed design is deferred to implementation.

### Object safety

`TokenKind` and `Node` currently require `Clone + PartialEq`, which are not
object-safe. To support dynamic composition, these traits must be redesigned:
remove the `Clone`/`PartialEq` supertraits, use `Arc<dyn TokenKind>` for shared
ownership, and provide equality via a trait method (`fn eq(&self, other: &dyn
TokenKind>) -> bool`) rather than `PartialEq`. The same applies to `Node`.

### Multi-trait representation

`CanonicalKind` currently implements 19 capability traits (`CallToken`,
`IndexToken`, `PipeToken`, …). A single `Box<dyn TokenKind>` cannot represent
all 19 simultaneously. Options: a mega-trait combining all capabilities (every
plugin depends on all — rejected, not composable), separate boxed trait objects
per capability (token duplicated — wasteful), or a trait-object with downcast
for capability inspection. The right approach is an open design question for
implementation; the architecture requires that it be solved without a static
unified enum.

### Parser framework

`plugin-ext-parser`'s `BasicParser<K: TokenKind>`, `ParserContext<K>`, and
`ParserBehavior<K>` are statically generic. Moving to dynamic composition means
rewriting these from `K: TokenKind` generics to trait-object-based dispatch.
This is a framework rewrite, acknowledged as part of the cost.

### Self-registration

Rust has no built-in runtime self-registration. Options: the `inventory` crate
(linker-based, not portable to WASM), explicit registration in `contribute`
(the kit calls `ctx.register_plugin(...)` for each plugin — same as today, just
moved), or build-script codegen. The `contribute`-based approach is the
simplest and most portable; it keeps the kit as the source of truth for
curation while the framework provides the dynamic composition machinery.

### AST inspection without pattern matching

Consumers like `host-cli/main.rs` currently pattern-match on `KitNode::Literal`
/ `::Call` / `::Array` / …. With dynamic composition, pattern matching on a
unified enum is impossible. Consumers must use a visitor pattern, downcast, or
`dyn Any` inspection. The framework must provide an ergonomic inspection API;
the design is deferred to implementation.

### `plugin-lib-*` unification

`plugin-lib-*` crates currently have stub `Plugin` impls (`// TODO`) while real
registration is via `Module` structs + `ModuleHost::with()`. For kits to curate
stdlib plugins, the `Plugin` impls must be completed so that `contribute`
registers the `Module` structs into the host. This is a completion task, not a
redesign.

### `handl-platform` value model

`platform/src/value.rs` embeds `kit-type` concrete types (`Term`, `Value`,
`synth`, `eval`). Making the typechecker a replaceable plugin means the
platform's value model must become generic over type representations, not
hard-coded to `kit-type`. This is a deep change scoped as part of Phase 3.

### Kit composition

`KitManifest` is currently a flat `plugins: Vec<String>` with no kit-to-kit
composition. The ADR requires that `kit-server` extends `kit-config`. This
needs either a `contribute`-chaining mechanism (a kit registers another kit,
like `kit-hello` registers `CanonicalKit`) or a manifest `extends` field. The
`contribute`-chaining approach already works today and is the simplest path.

### Registration ordering

Lexer rules need ordering (`///` before `//`); grammar rules have binding
powers. Self-registration must support priority or ordering. The framework
registry must accept an ordering hint from each plugin.

### Capability trait placement

`PipeToken`, `HandleToken`, `MatchToken`, etc. are handl-specific capability
traits. They live in `plugin-grammar-*` crates (handl-specific plugins), not in
the framework. The framework's registries must be generic enough to accept
plugins with arbitrary capability traits — the design of this genericism is
deferred to implementation.

## Related work

- ADR-0001: Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical
  Kit
- ADR-0033: Unified Plugin Crate Naming
- ADR-0034: Zero-Monolith, Plugin-Driven Platform (superseded in part by this
  ADR's dynamic-composition decision)
- ADR-0001 (Remaining): Canonical Token/Grammar/Value Plugin Cleanup
