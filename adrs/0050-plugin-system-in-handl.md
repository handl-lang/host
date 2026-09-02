# ADR-0050: Plugin System in handl Itself

## Status

Proposed

Refines [ADR-0047 — Meta Functions as `plugin-lib-*` Crates](./0047-meta-functions-as-plugin-lib-crates.md)
(hands the meta functions to handl as callable modules, which this ADR then *uses* from handl).
Related: [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md),
[ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md).

## Context

The platform's composition model — `Plugin`, `Operation`, `Behavior<Op>`, `RuleAcceptor`,
`ContributionContext`, `PluginRegistry`, `Host` — is today implemented entirely in Rust
(`crates/platform/src/`). Concrete operations (`Format`, `Lint`, `Eval`, `Compile`, …) are
Rust traits with Rust rule acceptors (`BasicFormatter`, `BasicLinter`, …). Rules are Rust
structs implementing `Behavior<Op>`.

This works, but it puts the *first application of handl* — its own tooling — in Rust. Every
new format/lint rule is a Rust crate, a recompile, and a host edit. The language's own
toolchain cannot be extended by writing handl. That is the opposite of the platform's stated
goal ("every concept is a `Plugin`, every tool is a `Host` that composes plugins") when the
plugins themselves can only be Rust.

The question this ADR answers: **can the plugin system itself — registry, contribution,
dispatch, the rule acceptors, and the fmt/lint rules — be handl, with Rust reduced to a
permanent bootstrap kernel?**

### What handl already has (verified)

- `if` — conditionals.
- Named bindings + lambdas → recursion.
- `str`, `array`, `object`, `fn` modules — string building, collection traversal, higher-order combinators.
- AST payload is `serde_json::Value` — the AST is already JSON-shaped at the Rust boundary.
- `plugin-loader-wasm` / `plugin-loader-native` — handl scripts can already be loaded as plugins.

handl is **not** missing control flow, recursion, or string building. The gap is narrower
than it first appears.

### What handl is missing (verified)

1. **AST exposure.** There is no `ast.parse(source)` module callable from handl today.
   `plugin-lib-parse`'s `LibraryPlugin` registers nothing (its `contribute` is empty). The
   parser is Rust (`plugin-engine-parse-pratt`) and its output does not cross into handl.
2. **A rule-contribution path.** handl scripts can be loaded as plugins, but there is no
   bridge that lets a handl script contribute `FormatBehavior`/`LintBehavior` rules into a
   `BasicFormatter`/`BasicLinter` acceptor, nor an equivalent acceptor written in handl.
3. **A binding façade.** `ModuleRuntime` + `ModuleRule` is the binding mechanism, but
   exposing a new Rust library today means writing a full `plugin-lib-*` crate per library.
   There is no generic "register this Rust fn as that handl module function" primitive.

### The two permanent Rust pieces

A self-hosted toolchain does not mean zero Rust. Every self-hosted language keeps a kernel
runtime in a host language (CPython in C, V8 in C++, rustc in Rust with LLVM in C++). Porting
the *evaluator* to handl would only make handl slower running handl — no gain. The win is
porting the **toolchain** (parser, formatter, linter, type-checker), not the engine.

This ADR therefore fixes two permanent Rust pieces and moves everything above them into handl:

1. **Bootstrap engine** — `plugin-engine-eval-*` (tree-walk / bytecode / cranelift). handl
   cannot run without a runtime. The engine is the kernel that stays Rust.
2. **Binding lib** — a generic extern façade so any Rust crate is exposed as a handl module
   through one uniform primitive. Mostly exists (`ModuleRuntime` + `ModuleRule`); needs a
   clean generic surface so a new Rust binding is not a full `plugin-lib-*` crate.

Everything above the line — stdlib, the plugin system, parser, fmt, lint, rules — becomes
handl.

## Decision

### 1. Two permanent Rust layers; everything above is handl

```
Rust (permanent):
  1. bootstrap engine   — plugin-engine-eval-* (tree-walk/bytecode/cranelift)
  2. binding lib        — generic extern façade over ModuleRuntime + ModuleRule
─────────────────────────────────────────────────────────────────────────
handl (everything above the line):
  stdlib          — str, array, object, fn, ... (uses bindings where needed)
  plugin system   — registry, contribute, dispatch, host.load (pure handl)
  parser          — port to handl (phase 2; may use bindings for lexer primitives)
  fmt + lint      — rule acceptors + per-kind rules (pure handl, extensible)
```

The engine stays Rust because porting it yields only a slower handl. The binding lib stays
Rust because it is the seam between handl and the host language's library ecosystem — that
seam is inherently host-language code. Nothing else needs to be Rust.

### 2. The plugin system is a handl module, not a Rust crate

The generic plugin system — `PluginRegistry`, `ContributionContext`, `RuleAcceptor`,
`Operation`, `Behavior`, `Host.load` fixpoint — is implemented as a **handl module**
(`plugin.*`) backed by handl data and handl functions. No new `plugin-lib-plugin` Rust crate
holds the *logic*; the only Rust involved is the binding façade that exposes whatever host
primitives the handl module needs (e.g. a stable `ast.parse` callable).

The Rust trait model maps to handl data with no loss of expressiveness, because handl is
JSON + functions:

| Rust                                | handl                                              |
| ----------------------------------- | -------------------------------------------------- |
| `Plugin` trait                      | object `{id, capabilities, contribute, run}`       |
| `Operation` marker type             | string id (`"fmt"`, `"lint"`)                       |
| `Behavior<Op>` trait                | object `{kind, apply}`; `apply` is a fn mutating a request |
| `RuleAcceptor` trait                | a plugin whose `run` dispatches to behaviors by `kind` |
| `PluginRegistry`                    | handl object: map of capability → plugin            |
| `ContributionContext`               | handl object passed to `contribute`                |
| `Host` + fixpoint load              | handl `host.load` function: register → contribute → resolve → repeat to fixpoint |

Traits are not needed on the handl side because dispatch is by string `kind` over JSON
requests, which is exactly what the Rust `BasicFormatter`/`BasicLinter` already do
internally (`for rule in &self.rules { if rule.kind() == kind { rule.apply(&mut req)?; } }`).
The handl version is the same loop written in handl.

### 3. `fmt` and `lint` are handl rule acceptors with per-kind rule files

`fmt` and `lint` become handl modules, each providing a rule acceptor that dispatches by
`AstNode.kind` over the AST-as-JSON. Rules are handl files — one per AST kind — that
register into the acceptor:

```
# rules/fmt/call.hnl   — formats a `call` node
# rules/fmt/var.hnl    — formats a `var` node
# rules/lint/unused.hnl — lints for unused bindings
```

Adding a rule is adding a handl file. No Rust change, no recompile, no host edit. This is
the extensibility property the Rust model promised but could only deliver at Rust-compile
time.

The `FormatRequest` / `LintRequest` contracts (currently undesigned —
`plugin-fmt-expression` returns `NotImplemented`) are defined **as handl data shapes** while
the first rules are written. Early churn is expected and accepted; the contract hardens as
the rule set grows.

### 4. `handle fmt|lint` dispatches to the handl tooling kit

`host-cli` gains `fmt` and `lint` subcommands. They are thin: they eval the handl "tooling
kit" (a kit that loads the `plugin`/`fmt`/`lint` modules and the rule files) and call the
handl `fmt`/`lint` entry points with the target source. The CLI does not implement
formatting or linting; it hosts the handl that does.

### 5. Build order

1. **Binding lib façade (Rust).** Generic extern module registration; one primitive to
   expose a Rust fn as a handl module function.
2. **Expose `ast.parse` / `ast.lex` via the façade.** Parser stays Rust for now; this makes
   the end-to-end loop work (handl can read a source file's AST).
3. **handl `plugin` module.** Registry, contribute, dispatch, `host.load` fixpoint — pure
   handl, mirroring the Rust model in the table above.
4. **handl `fmt` + `lint` modules.** Rule acceptors with kind-dispatch over AST JSON.
5. **handl rule files.** One per AST kind; add without touching the language.
6. **Wire `handle fmt|lint` in `host-cli`.** Eval the tooling kit, call the entry points,
   print results.
7. **(Later phase) Port the parser to handl.** Rust shrinks to engine + bindings. The
   `ast.parse` binding becomes a fallback / fast path, not the only path.

Steps 1–2 are the bootstrap. Steps 3–6 are the deliverable. Step 7 is the eventual
self-hosting reward, not the starting line.

## Consequences

### Positive

- **First application of handl is handl.** The language's own toolchain — formatter,
  linter, and the plugin system that hosts them — is written in the language. This is the
  strongest possible dogfooding and the clearest proof of the platform's composability
  claim.
- **Rules are extensible without touching the language.** A new format or lint rule is a
  `.hnl` file. No Rust crate, no recompile, no host edit. The Rust model promised this at
  Rust-compile time; the handl model delivers it at file-add time.
- **The plugin system is agnostic / decoupled / extensible / modular / composable /
  replaceable** — because it is handl data + functions, and rules are just more handl
  registering into it. Swapping the whole plugin system is swapping a handl module.
- **Rust shrinks to a permanent, small kernel.** Engine + bindings. Everything above is
  auditable, editable, and replaceable as handl. The surface area of "things you must know
  Rust to change" becomes minimal.
- **The contract hardens under real use.** `FormatRequest` / `LintRequest` get designed
  while writing the first real rules, against real AST shapes, in the language that will
  consume them.
- **Bindings unlock the Rust ecosystem.** handl stdlib can use mature Rust crates (unicode,
  regex, …) where handl stdlib is thin, without reimplementing them. Self-hosting the
  toolchain logic does not require reimplementing every library.

### Trade-offs

- **A bootstrap bridge must be written and maintained.** The binding façade and the
  `ast.parse`/`ast.lex` exposure are Rust and must track engine/parser changes. This is
  small and stable compared to the toolchain, but it is non-zero.
- **Performance.** A tree-walk-evaluated formatter over large files will be slower than a
  Rust formatter. Mitigations: bytecode/cranelift engines for the tooling kit, or a fast
  Rust `ast.parse` path that returns a compact JSON shape. Not a day-1 concern; revisit
  when the rule set is complete and real files are being formatted.
- **Self-hosting trap.** Every handl syntax change can break the formatter, which is
  written in the language just changed. True of every self-hosted toolchain (gofmt, rustfmt
  partially, GHC) and survivable: the bootstrap engine and the Rust `ast.parse` fallback
  keep the toolchain runnable while handl syntax is in flux.
- **Contract churn early.** `FormatRequest` / `LintRequest` are designed alongside the
  first rules. Expect revisions until the rule set covers the AST. This is intentional —
  designing the contract in an immature language against real rules is more honest than
  designing it in the abstract — but it means early rule files will be rewritten.
- **Parser port (step 7) is deferred, not free.** Full self-hosting (no Rust parser) is a
  later phase with its own ADR. This ADR delivers a self-hosted *toolchain* on top of a
  Rust parser; the parser port is the next milestone, not part of this decision.

## References

- [ADR-0047 — Meta Functions as `plugin-lib-*` Crates](./0047-meta-functions-as-plugin-lib-crates.md)
  — provides `lex.lex` / `parse.parse` / `eval.eval` as handl-callable modules, which this
  ADR's binding façade generalizes and which the handl `plugin` module uses.
- [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  — the data/runtime split that makes the AST JSON-shaped and the engine pluggable.
- [ADR-0046 — Capability Discipline at the Language Level](./0046-capability-discipline-at-the-language-level.md)
  — capability discipline that the handl plugin system inherits.
- `crates/platform/src/plugin.rs` — the Rust `Plugin` / `PluginRegistry` model this ADR
  ports to handl.
- `crates/platform/src/format.rs`, `crates/platform/src/lint.rs` — the Rust
  `BasicFormatter` / `BasicLinter` rule acceptors the handl `fmt` / `lint` modules mirror.
- `crates/platform/src/plugin_model.rs` — `Contribution`, `RuleAcceptor`, `Operation`,
  `Behavior`, `ContributionContext` — the composition primitives ported to handl data.
