# Platform architecture

The handl platform is a family of Rust crates organised around one goal:
every concept is a `Plugin`, every tool is a host that composes
plugins, and a language is data the engines dispatch on.

## The public repo layers

```text
kernel/                        # the Language schema + shared values
  handl-calculus              #   the Term/Model arrow-calculus IR
  handl-language               #   the Language data schema (ADR-0044);
                               #   re-exports the Term kernel
  handl-value                  #   the tagged Value model (TaggedValue)
  handl-ops                    #   the op ABI (Op/OpRegistry/OpContext)
  handl-ext                    #   the dynamic-extension ABI (ADR-0149)
  handl-defining-handl         #   the self-hosting corpus
  handl-lib-*                  #   remaining floor substrate packages
                               #   (ops, prim, prim-handl)
engine/                        # hnl-engine — engine-side helpers over the calculus
engine-*/                      # generic engines — one repo per operation
  tool-lex, tool-lex-pattern
  tool-parse, tool-parse-pratt
  tool-transform, tool-transform-rewrite
  exec, exec-tree-walk, exec-bytecode,
  exec-cranelift
  tool-fmt, tool-lint, tool-typecheck
host/                          # the platform workspace
  crates/platform              #   handl-platform — contracts + DefaultHost
  crates/plugin-ext-*          #   operation contract surfaces
  crates/plugin-applier-*      #   config → HostConfig resolution
  crates/plugin-load*          #   plugin loaders (manifest, native)
  crates/plugin-lib-*          #   native/runtime library modules
  crates/handl-runtime         #   runtime coordination, eval dispatch
  crates/handl-kit-*           #   kit data assembly
  crates/host-tooling          #   semantic-token/LSP helpers
kit-prelude-lang/              # the canonical eval-path Language as YAML data
host-tensor/                   # dense n-d tensor functions for runtimes
```

| Layer | Responsibility |
| --- | --- |
| `kernel` | The `Language` data schema, the `Term`/`Model` calculus (`handl-calculus`), the op ABI (`handl-ops`), and the shared value model. `handl-language` defines the fragment schema and re-exports the `Term` kernel; `handl-value` provides `TaggedValue` (including the tensor scalar); `handl-lib-*` are the remaining floor substrate packages — the rest of the floor libs live in their own `lib-*` repos. Self-contained: no `engine`/`host` deps. |
| `engine` (`hnl-engine`) | Engine-side helpers over the arrow calculus — the `Term`/`Model` data itself lives in the kernel's `handl-calculus`. |
| `engine-*` | One abstraction crate per operation (`lex`, `parse`, `transform`, `exec`, `typecheck`) plus concrete engines — the tree-walk reference interpreter, the bytecode VM, the Cranelift JIT, the rewrite engine, and the `fmt`/`lint`/`typecheck` toolchain engines. Generic: they dispatch on `Language` data, never on hardcoded constructs. |
| `handl-platform` | The composition primitives (`Plugin`, `Operation`, `Behavior`, `RuleAcceptor`, `Contribution`), `DefaultHost`, `PluginRegistry`, `Capability`/`CapabilityRequest`, `Kit`/`KitManifest`, `HostConfig`, `OptionsPlugin`, and the built-in operation contracts (`Format`, `Lint`, `Build`, `Compile`, `Eval`). |
| `handl-plugin-ext-*` | Published capability **contract surfaces** and default `RuleAcceptor` plugins (`diag`, `detect`, `fmt`, `lint`, `eval`, `kit`, `compile`, `plugin`, `refinement`, `codegen`, `test`, `typecheck`). |
| `handl-plugin-applier-*` | Read `.handl.toml` into a `HostConfig` and apply it — `ApplierFactory` / `DefaultApplierPlugin` register kits, plugins, options, and excludes through capabilities. |
| `handl-plugin-load*` | Plugin loaders — manifests, and the native cdylib loader. |
| `handl-plugin-lib-*` | Library plugins that register native/runtime modules (`stdlib`, `handl`, `scheduler`, `tag`, …). |
| `handl-runtime` | Runtime coordination — desugaring and eval-engine dispatch. |
| `handl-kit-*` | Kit data assembly — merge `Language` fragments and manifests into loadable kits. |
| `kit-prelude-lang` | The canonical eval-path `Language` — the published YAML fragments (`tokens`, `grammar`, `eval`, `types`, `ast`, `caps`, `options`, `lexer_rules`, `transforms`, `globals`). |

## What is not published

The layer above the contracts is only partly public. Public today: the
kernel, the `engine`/`engine-*` family (the whole operation surface —
lex, parse, transform, exec, fmt, lint, typecheck — minus the private
NBE typecheck engine and the test runner), the platform workspace,
`kit-prelude-lang`, `host-tensor`, and a handful of `lib-*` packages
(`lib-char`, `lib-char-native`, `lib-desugar`, `lib-loader`,
`lib-str-handl`).

Not yet released: the `hnl` CLI and LSP hosts (`host-cli`, `host-lsp`),
the `ext-*` command extensions, the curated `kit-*` kits, and most of the
`lib-*` standard library. Those appear here only as a boundary: the
contracts they plug into are public, most of the implementations are not.
Two newer repos are also still private: `engine-eqsat`, the e-graph
**search** interpretation over kernel `Term`s (ADR-0174 phase P2), and
`compile-cuda`, a `compile-*` emitter lowering `lib-kernel-ir` terms to
CUDA C++.

## The product repos

The platform machinery above is what the published **products** are
built on. Each has its own docs section on this site:

| repo | what it is |
| --- | --- |
| [`graphopt`](https://github.com/handl-lang/graphopt) | The research stack — a categorical optimizer for neural-network computation graphs. Lifts models to the `Term` IR, saturates an e-graph under a law bank, verifies the result, and delivers a runnable artifact. Its discovery loop *mints* rewrite laws, runs them through a certification gauntlet, and learns to play the game the growing bank creates. |
| [`eqsat`](https://github.com/handl-lang/eqsat) | The equality-saturation substrate `graphopt` is built on — `mlopt-core` (the torch-free term IR, e-graph, certificates, law machinery, cost algebra) and `mlopt-orchestrator` (the backend-neutral `search`/`lower` pipeline), behind port traits. |
| [`gamekit`](https://github.com/handl-lang/gamekit) | The domain-free game layer — the `Board` protocol, episode/train/eval drivers, the player registry, scoring specs, and the benchmark harness (`benchkit`) any domain can pin. |
| [`host-tensor`](https://github.com/handl-lang/host-tensor) | Dense n-d `f64` tensor functions (`tensor.*`) for `.hnl` runtimes — a host-side plugin, not a language feature. |
| [`kit-prelude-lang`](https://github.com/handl-lang/kit-prelude-lang) | The canonical eval-path `Language` as YAML fragments — the language-as-data payload the engines dispatch on. |
| [`egraph`](https://github.com/handl-lang/egraph), [`discovery`](https://github.com/handl-lang/discovery) | The Rust ports of the same ideas — `handl-egraph`, a generic e-graph/equality-saturation substrate over the kernel's `Term`, and `handl-discovery`, the pure-compute legs of the law-discovery/gauntlet machinery composed over it. |

The split mirrors the platform's own layering: `eqsat`/`gamekit` are
domain-free substrates (like `handl-platform` is for hosts), `graphopt`
is the domain that composes them, and `host-tensor` is a host-side
component any runtime can compose. Where a product lowers into `.hnl`,
it goes through the same `Language`-as-data and `Term` machinery
described above — the optimizer's rewrite laws and the language's
transform rules are the same kind of object seen from two sides.

## The direction: one term, many interpretations

The accepted end-state (ADR-0174) tightens this layering into a single
pipeline: **a model is lifted once into Handl `Term`s; a search
interpretation optimizes them under verified laws; delivery
interpretations emit runnable artifacts.** Concretely:

- `engine-eqsat` is the e-graph search interpretation — saturation and
  extraction as one more engine beside tree-walk, bytecode and
  Cranelift, over the same kernel `Term`, not a second IR.
- `compile-*` emitters lower `Term`s to targets; `compile-cuda` is the
  first CUDA backend, emitting real CUDA C++ from `lib-kernel-ir`
  terms and declining uncovered ops by name.
- Pipelines are themselves terms: `lift >>> search >>> verify >>>
  deliver` is `Seq` composition, and `hnl run <script>.hnl` evaluates
  scripts whose stage-ops (`lift`, `search`, `verify`, `deliver`,
  `mint`, `gauntlet`, `bench`) are registered by kits. The stage
  protocol already exists — `python -m mlopt stage
  lift|search|verify|deliver` is the kit-mlopt JSON endpoint — and
  `kit-mlopt` is its ratified home. The game layer (`gamekit`,
  `discoverykit`, `benchkit`) becomes a meta tier of composable,
  user-writable scripts rather than a fixed driver.
- The research repos dissolve toward kits along the ratified map
  (ADR-0175): `graphopt`'s contents are headed for `kit-torch` /
  `kit-onnx`, and `mlopt` becomes `kit-mlopt` — a kit assembling
  adapters and stage-ops, not an engine with an IR of its own.

This is the agreed direction, still in flight — the repos in the
tables above remain the shipping surface today.

## Plugins are the only unit

`handl-platform` does not distinguish between a component, an extension,
a library, a target, and a capability. The only real distinction is
whether a plugin implements `RuleAcceptor` and, if so, what rule it
accepts. Rule plugins contribute `Behavior`s; kits curate sets of
plugins; executable hosts run them.

## Why this split

The kernel and platform layers intentionally stay small. They define the
calculus vocabulary, the `Language` schema, and the composition
primitives — nothing else. Concrete operations, library rules, and
targets live in separate crates because they can be added, swapped, or
excluded without changing the core. This lets a host be as small as a
formatter or as large as a language server.

A new crate is added only when it represents a distinct dependency or
consumer boundary. If two crates are always consumed together, they
belong in one crate. If one can be used without the other, they stay
split. Every crate boundary should correspond to a real dependency,
target, or consumer boundary.

Because the platform layer is the only shared contract, dependency
direction is always clear, and new operations can be added as new
`plugin-ext-*` contracts without changing `handl-platform`. Tools share
the same composition model, from a CLI to an LSP server to a test runner.

## The host as composer

A `DefaultHost` starts empty and ends as a fully composed tool. In
practice a host is assembled by an **applier** plugin
(`plugin-applier-bootstrap` selects one via `ApplierFactory`;
`plugin-applier-default`'s `DefaultApplierPlugin` is the shipped
implementation):

1. The applier reads `.handl.toml` into a `HostConfig`
   (`HostConfig::from_toml`).
2. Its `contribute` registers the kit loader, options plugin, and loader
   plugins, then loads each configured kit through the `kit/load`
   capability and each plugin manifest through `load`.
3. `[options]` entries are applied through `option/set`; `exclude`
   entries are scheduled via `ContributionContext::exclude_plugin`.
4. `DefaultHost::load` then makes a single registration-order pass over
   every plugin, calling `contribute` once each; rule contributions
   whose acceptor is not yet registered are deferred and retried as
   loading progresses.
5. `DefaultHost::run` dispatches a `CapabilityRequest` to the plugin
   that provides the requested capability.

`DefaultHost::resolve_composition` additionally validates the live
registry against the manifest-level resolver (`resolve`).

## Adding a new operation

To add a new operation such as `Hover`:

1. Define a `Hover` marker type implementing `Operation` (with its
   `Request` type).
2. Define a `HoverBehavior` implementing `Behavior<Hover>`.
3. Provide a `BasicHover` as a `RuleAcceptor<Rule = Box<dyn HoverBehavior>>`.
4. Register `BasicHover` in the host.

No platform change is required — `Operation` and `Behavior<Op>` are
public extension points in `handl-platform`.

## How a plugin becomes a capability

A rule plugin can contribute behavior to any loaded `RuleAcceptor`, and
any plugin can answer a `CapabilityRequest` in `run`. The host learns
about neither ahead of time — it discovers both during the
registration-order `contribute` pass:

```rust
impl Plugin for MyLintRules {
    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<BasicLinter>(Box::new(MyLintBehavior))?;
        Ok(())
    }
    // … name/capabilities/metadata/run as usual
}
```

The host does not know the rule exists. The plugin contributed it, and
the host ran it when the corresponding capability was requested.
