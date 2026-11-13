# Platform architecture

The handl platform is a family of Rust crates organised around one goal:
every concept is a `Plugin`, every tool is a host that composes
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
kit-prelude-lang/              # the canonical eval-path Language as YAML data
host-tensor/                   # dense n-d tensor functions for runtimes
egraph/, discovery/            # equality-saturation + discovery substrates
```

| Layer | Responsibility |
| --- | --- |
| `kernel` | The calculus vocabulary — `Term`/`Value` shapes, the `Language` data schema, and the composition primitives (`Plugin`, `Operation`, `Behavior`, `RuleAcceptor`, `Contribution`). Depends on nothing in the org. |
| `engine-*` | One abstraction crate per operation (`lex`, `parse`, `transform`, `exec`) plus concrete engines (e.g. the tree-walk interpreter, and the published `engine-fmt` / `engine-lint` / `engine-typecheck` toolchain engines). Generic: they dispatch on `Language` data, never on hardcoded constructs. |
| `handl-platform` | `DefaultHost`, `PluginRegistry`, `Capability`/`CapabilityRequest`, `Kit`/`KitManifest`, `HostConfig`, `OptionsPlugin`, and the built-in operation contracts (`Format`, `Lint`, `Build`, `Compile`, `Eval`). |
| `handl-plugin-ext-*` | Published capability **contract surfaces** and default `RuleAcceptor` plugins (`diag`, `detect`, `fmt`, `lint`, `eval`, `kit`, `compile`, `plugin`, `refinement`, `sdkgen`, `test`, `typecheck`). |
| `handl-plugin-applier-*` | Read `.handl.toml` into a `HostConfig` and apply it — `ApplierFactory` / `DefaultApplierPlugin` register kits, plugins, options, and excludes through capabilities. |
| `handl-plugin-load*` | Plugin loaders — manifests, and the native cdylib loader. |
| `handl-plugin-lib-*` | Library plugins that register native/runtime modules (`stdlib`, `handl`, `scheduler`, `tag`, …). |
| `handl-runtime` | Runtime coordination — desugaring and eval-engine dispatch. |
| `handl-kit-*` | Kit data assembly — merge `Language` fragments and manifests into loadable kits. |
| `kit-prelude-lang` | The canonical eval-path `Language` — the published YAML fragments (`tokens`, `grammar`, `eval`, `types`, `ast`, `caps`, `options`, `lexer_rules`, `transforms`, `globals`). |

## What is not published

The layer above the contracts is only partly public. Public today: the
kernel, the platform workspace, the generic `engine-*` repos (including
the `engine-fmt` / `engine-lint` / `engine-typecheck` toolchain engines),
`kit-prelude-lang`, `host-tensor`, and a handful of `lib-*` packages
(`lib-char`, `lib-char-native`, `lib-desugar`, `lib-loader`,
`lib-str-handl`).

Not yet released: the `hnl` CLI and LSP hosts (`host-cli`, `host-lsp`),
the `ext-*` command extensions, the curated `kit-*` kits, and most of the
`lib-*` standard library. Those appear here only as a boundary: the
contracts they plug into are public, most of the implementations are not.

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
