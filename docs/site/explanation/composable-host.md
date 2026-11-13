# The composable host

`handl-platform` is intended to be the shared, runtime-agnostic kernel for the handl language. The composable host is the idea that almost everything in that kernel can be reduced to one concept: the `Plugin`.

## Why a single plugin model

Earlier versions of the platform tried to keep several overlapping ideas apart: `Component`, `Plugin`, `Extension`, `Target`, `Library`, and `Kit`. Each had its own registry and lifecycle. In practice, the only meaningful distinction was whether a thing accepted contributions from other things.

A library simply registers more plugins. A target adds target-specific operations and options. A plugin is already capable of being executed. The old model created a lot of plumbing for very little benefit.

The composable host collapses these into a single object-safe `Plugin` trait (abridged — see rustdoc):

```rust
pub trait Plugin: Send + Sync + Any {
    fn id(&self) -> &str { self.name() }
    fn name(&self) -> &str;
    fn version(&self) -> &str { "0.0.0" }
    fn capabilities(&self) -> &[CapabilityId];
    fn requires(&self) -> &[CapabilityId] { &[] }
    fn optional(&self) -> &[CapabilityId] { &[] }
    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata>;

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> { Ok(()) }
    fn run(&self, request: &CapabilityRequest, ctx: &mut PluginContext)
        -> Result<CapabilityResponse, PlatformError>;
}
```

A `RuleAcceptor` is a sub-trait for plugins that accept contributions:

```rust
pub trait RuleAcceptor: Plugin {
    type Rule: Contribution;
    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError>;
}
```

A `Library`, a `Target`, an `Operation`, and an executable all become the same kind of thing. The only thing that changes is which methods they implement.

## The host as a composer

`DefaultHost` is not a concrete type full of hard-coded fields. It holds two registries — `PluginRegistry` and `KitRegistry` — and composes plugins through them. When `DefaultHost::load` runs, it makes a **single pass over the registered plugins in registration order**, calling each plugin's `contribute` exactly once. Plugins registered *during* the pass are visited later in the same pass, so loading one library can still bring in dozens of other plugins without the host knowing about them ahead of time.

Rule contributions whose `RuleAcceptor` is not registered yet are **deferred**: they are queued and retried as loading progresses (`flush_pending_rules` after each `contribute`, plus a final flush). A rule whose acceptor never appears fails `load` with `MissingRuleAcceptor` — deferred is not the same as ignored.

The composer also holds the `OptionsPlugin`, which is itself a `RuleAcceptor` for `OptionSchema`. Other plugins contribute their option schemas, and `OptionsPlugin::resolve` merges schema defaults with overrides set through its `option/set` capability (which is how `HostConfig`'s `[options]` table is applied). Because options live in the same contribution graph as everything else, a plugin can introduce and consume a new tunable without touching any other crate.

## The AST as a shared value type

`AstNode` is a single, generic, composable value type:

```rust
pub struct AstNode {
    pub kind: String,
    pub span: Option<Span>,
    pub payload: serde_json::Value,
}
```

There is no `Ast` plugin. Instead, the `Basic*` operation acceptors (`BasicFormatter`, `BasicLinter`, `BasicBuilder`, `BasicCompiler`, `BasicEvaluator`) collect `Behavior` rules keyed by `AstNode.kind`. A plugin introduces a new language construct by contributing `Behavior` rules to each operation for the same `AstNode.kind`. Because the AST is not a fixed enum, adding a new kind does not require a platform change.

## Why this is enough

The four pieces above — `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` — cover libraries, targets, operations, rules, options, and the AST. Because the host is a composer, any combination of plugins can be loaded into the same process. Because `RuleAcceptor` and `Behavior` are generic, new operations can be added in new `handl-plugin-ext-*` crates without changing `handl-platform`. Because the CLI, the LSP server, the test runner, and the REPL are all just executable hosts, they all share the same composition rules.

This is the composable host: one plugin, one registry, one loading pass, and every tool built from the same pieces.
