# The composable host

`handl-platform` is intended to be the shared, runtime-agnostic kernel for the handl language. The composable host is the idea that almost everything in that kernel can be reduced to one concept: the `Plugin`.

## Why a single plugin model

Earlier versions of the platform tried to keep several overlapping ideas apart: `Component`, `Plugin`, `Extension`, `Target`, `Library`, and `Kit`. Each had its own registry and lifecycle. In practice, the only meaningful distinction was whether a thing accepted contributions from other things.

A library simply registers more plugins. A target adds target-specific operations and options. A plugin is already capable of being executed. The old model created a lot of plumbing for very little benefit.

The composable host collapses these into a single object-safe `Plugin` trait:

```rust
pub trait Plugin: Send + Sync + 'static + std::any::Any {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn metadata(&self) -> serde_json::Value;
    fn requires(&self) -> &[CapabilityId] { &[] }
    fn optional(&self) -> &[CapabilityId] { &[] }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> { Ok(()) }
    fn run(&self, request: &CapabilityRequest, ctx: &mut PluginContext)
        -> Result<CapabilityResponse, PlatformError>
    { Err(PlatformError::NotImplemented) }
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

A `Host` is not a concrete type full of hard-coded fields. It is a composer with one primary registry: `PluginRegistry`. When a host loads, it resolves plugin dependencies, runs each plugin's `contribute` method, and repeats that process until no new plugins appear.

This fixed-point loading means that loading one library can bring in dozens of other plugins, and the host does not need to know about them ahead of time.

The composer also holds the `Options` plugin, which is itself a `RuleAcceptor<OptionSchema>`. Other plugins contribute their options, and the `Options` plugin resolves the final values from kit defaults, host configuration, and user overrides. Because options live in the same contribution graph as everything else, a plugin can introduce and consume a new tunable without touching any other crate.

## The AST as a shared value type

`AstNode` is a single, generic, composable value type:

```rust
pub struct AstNode {
    pub kind: String,
    pub span: Option<Span>,
    pub payload: serde_json::Value,
}
```

There is no `Ast` plugin. Instead, `Formatter`, `Linter`, `Typechecker`, `Parser`, `Lexer`, and `Diagnostics` are all `RuleAcceptor` plugins. A plugin introduces a new language construct by contributing `Behavior` rules to each of those operations for the same `AstNode.kind`. Because the AST is not a fixed enum, adding a new kind does not require a platform change.

## Why this is enough

The four pieces above — `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` — cover libraries, targets, operations, rules, options, the AST, and data types. Because the host is a composer, any combination of plugins can be loaded into the same process. Because `RuleAcceptor` and `Behavior` are generic, new operations can be added in new `handl-plugin-ext-*` crates without changing `handl-platform`. Because the CLI, the LSP server, the test runner, and the REPL are all just executable hosts, they all share the same composition rules.

This is the composable host: one plugin, one registry, one loading algorithm, and every tool built from the same pieces.
