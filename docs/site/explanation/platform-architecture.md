# Platform architecture

The handl platform is a family of Rust crates organised around one goal:
every concept is a `Plugin`, every tool is a `Host` that composes
plugins, and a language is data the engines dispatch on.

## The public repo layers

```text
kernel/                        # the calculus + Language schema
  hnl-engine                   #   Term/Value calculus, composition primitives
  handl-language               #   the Language data schema (ADR-0044)
  handl-value                  #   the shared tagged Value model
  handl-ext, handl-lib-*       #   kernel substrate packages
engine-*/                      # generic engines — one repo per operation
  hnl-engine-lex, -lex-pattern
  hnl-engine-parse, -parse-pratt
  hnl-engine-transform
  hnl-engine-exec, -exec-tree-walk
host/                          # the platform workspace
  crates/platform              #   handl-platform — contracts + DefaultHost
  crates/plugin-ext-*          #   operation contract surfaces
  crates/plugin-applier-*      #   config → HostConfig resolution
  crates/plugin-load*          #   plugin loaders (manifest, native)
  crates/plugin-lib-*          #   native/runtime library modules
  crates/handl-runtime         #   runtime coordination, eval dispatch
  crates/handl-kit-*           #   kit data assembly
  crates/host-tooling          #   semantic-token/LSP helpers
  crates/handl                 #   the facade
kit-prelude-lang/              # the canonical eval-path Language as YAML data
host-tensor/                   # dense n-d tensor functions for runtimes
egraph/, discovery/            # equality-saturation + discovery substrates
```

| Layer | Responsibility |
| --- | --- |
| `kernel` | The calculus vocabulary — `Term`/`Value` shapes, the `Language` data schema, and the composition primitives (`Plugin`, `Host`, `Operation`, `Behavior`, `RuleAcceptor`, `Contribution`). Depends on nothing in the org. |
| `engine-*` | One abstraction crate per operation (`lex`, `parse`, `transform`, `exec`) plus concrete engines (e.g. the tree-walk interpreter). Generic: they dispatch on `Language` data, never on hardcoded constructs. |
| `handl-platform` | `DefaultHost`, `PluginRegistry`, `Capability`/`CapabilityRequest`, `Kit`/`KitManifest`, `Options`, and the built-in operation contracts (`Format`, `Lint`, `Compile`, `Eval`). |
| `handl-plugin-ext-*` | Published capability **contract surfaces** and default `RuleAcceptor` plugins (`diag`, `detect`, `fmt`, `lint`, `eval`, `kit`, `compile`, `plugin`, `refinement`, `sdkgen`, `test`, `typecheck`). A contract is public even where the engine behind it is not yet released. |
| `handl-plugin-applier-*` | Read `.handl.toml` / CLI args / env into a `HostConfig` and apply it to a `Host`. |
| `handl-plugin-load*` | Plugin loaders — manifests, and the native cdylib loader. |
| `handl-plugin-lib-*` | Library plugins that register native/runtime modules (`stdlib`, `handl`, `scheduler`, `tag`, …). |
| `handl-runtime` | Runtime coordination — desugaring and eval-engine dispatch. |
| `handl-kit-*` | Kit data assembly — merge `Language` fragments and manifests into loadable kits. |
| `kit-prelude-lang` | The canonical eval-path `Language` — the published YAML fragments (`tokens`, `grammar`, `eval`, `types`, `ast`, `caps`, `options`, `lexer_rules`, `transforms`, `globals`). |

## What is not published

The layer above the contracts — the `hnl` CLI and LSP hosts, the `ext-*`
command extensions, the release toolchain engines (formatter, linter,
typechecker, test runner, compilers), the curated `kit-*` kits, and the
`lib-*` standard library — is not yet released. It appears here only as a
boundary: the contracts it plugs into are public, the implementations are
not.

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

A `Host` starts empty and ends as a fully composed tool. It does this by:

1. Loading `HostConfig` from a `HostApplier`.
2. Merging kit manifests into `PluginRegistry`.
3. Resolving `requires` and `optional` dependencies.
4. Running `Plugin::contribute` in topological order.
5. Repeating steps 3–4 until no new plugins appear.
6. Resolving `Options` from merged schemas and overrides.

After this, `Host::execute` dispatches a `CapabilityRequest` to the
plugin that provides the requested capability.

## Adding a new operation

To add a new operation such as `Hover`:

1. Define a `Hover` marker type implementing `Operation`.
2. Define a `HoverBehavior` implementing `Behavior<Hover>`.
3. Provide a `BasicHover` as a `RuleAcceptor<Rule = HoverBehavior>`.
4. Register `BasicHover` in the host.

No platform change is required — `Operation` and `Behavior<Op>` are
public extension points in `handl-platform`.

## How a plugin becomes a capability

A rule plugin can contribute behavior to any loaded `RuleAcceptor`, and
any plugin can answer a `CapabilityRequest` in `run`. The host learns
about neither ahead of time — it discovers both during the fixed-point
`contribute` pass:

```rust
impl Plugin for LintPlugin {
    fn id(&self) -> &str { "lint" }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<BasicLinter>(Box::new(NoUnusedImports))?;
        Ok(())
    }
}
```

The host does not know `lint` exists. The plugin contributed the rule,
and the host ran it when the `lint` capability was requested.
