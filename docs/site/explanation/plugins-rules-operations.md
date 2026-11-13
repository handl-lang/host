# Plugins, rules, and operations

The handl platform reduces almost every runtime concept to a single `Plugin` trait. This page explains how `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` relate to one another. Signatures below are abridged — see the `handl-platform` rustdoc for the full surface.

## Plugin

A `Plugin` is the universal, object-safe trait. Every loaded component implements it:

```rust
pub trait Plugin: Send + Sync + Any {
    fn id(&self) -> &str { self.name() }
    fn name(&self) -> &str;
    fn version(&self) -> &str { "0.0.0" }
    fn capabilities(&self) -> &[CapabilityId];
    fn requires(&self) -> &[CapabilityId] { &[] }
    fn optional(&self) -> &[CapabilityId] { &[] }
    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata>;
    fn manifest(&self) -> Option<&PluginManifest> { None }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> { Ok(()) }
    fn run(&self, request: &CapabilityRequest, ctx: &mut PluginContext)
        -> Result<CapabilityResponse, PlatformError>;
}
```

A plugin can be a library, a target, an operation, or a rule. The only thing that changes is which methods it overrides and whether it also implements `RuleAcceptor`.

## RuleAcceptor

A `RuleAcceptor` is a plugin that accepts contributions:

```rust
pub trait RuleAcceptor: Plugin {
    type Rule: Contribution;
    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError>;
}
```

`RuleAcceptor` is not object-safe, but it is used only during `contribute`. The host downcasts to the concrete type at load time, so rule contribution is type-safe.

## Operation

An `Operation` is a marker trait for a cross-cutting concern, parameterized by the request type its behaviors see:

```rust
pub trait Operation: Send + Sync + 'static {
    type Request;
}
```

`handl-platform` ships the built-in markers `Format`, `Lint`, `Build`,
`Compile`, and `Eval` (each with a `*Request` type and a `*Behavior`
trait, plus a `Basic*` `RuleAcceptor` that collects the contributed
rules). Further operation surfaces live outside the platform — e.g.
`Load` in `plugin-load` and `Refine` in `plugin-ext-refinement`.
Toolchain entry points such as `typecheck` are *capabilities* served by
`plugin-ext-*` plugins, not `Operation`s. Adding a new operation does
not require a `handl-platform` change.

## Behavior

A `Behavior` is a rule that a plugin contributes to a specific `Operation` for a specific `AstNode` kind:

```rust
pub trait Behavior<Op: Operation>: Send + Sync + 'static {
    fn kind(&self) -> &str;
    fn apply(&self, op: &mut Op::Request) -> Result<(), PlatformError>;
}
```

For example, a formatter for `comment` nodes implements `Behavior<Format>` with `kind() == "comment"`. A lint rule for `import` statements implements `Behavior<Lint>` with `kind() == "import"`.

## ContributionContext

`ContributionContext` is the type-safe API for `Plugin::contribute`:

```rust
pub struct ContributionContext<'a> { /* plugins + kits registries */ }

impl<'a> ContributionContext<'a> {
    pub fn register_plugin(&mut self, plugin: impl Plugin + 'static) -> Result<(), PlatformError>;
    pub fn contribute<P: RuleAcceptor>(&mut self, rule: P::Rule) -> Result<(), PlatformError>;
    pub fn contribute_exec_control(&mut self, rule: Arc<dyn ExecControl>) -> Result<(), PlatformError>;
    pub fn run(&mut self, request: &CapabilityRequest) -> Result<CapabilityResponse, PlatformError>;
    pub fn exclude_plugin(&mut self, id: String);
    // … plus resolve / get_plugin_mut / has_plugin_id
}
```

`register_plugin` is used by library and target plugins. `contribute` is used by rule plugins. The type parameter `P` is checked at compile time, so a `Format` rule cannot accidentally be contributed to a `BasicLinter`.

If the acceptor is not registered yet, the rule is **deferred**: it is queued and retried as `DefaultHost::load` progresses, and applied as soon as the acceptor appears. Contribution order does not matter.

## What each kind of plugin does

| Kind      | Implements                            | Typical `contribute` work                                             |
| --------- | ------------------------------------- | --------------------------------------------------------------------- |
| Library   | `Plugin`                              | Calls `register_plugin` to load more plugins.                         |
| Target    | `Plugin`                              | Registers target-specific plugins and contributes `OptionSchema`s.    |
| Operation | `Plugin + RuleAcceptor`               | Accepts rules and executes them when a capability is requested.       |
| Rule      | `Plugin`                              | Calls `contribute::<P>(rule)` to add a rule to an operation.          |
| Options   | `Plugin + RuleAcceptor<OptionSchema>` | `OptionsPlugin` resolves option values from contributed schemas.      |

## Why not separate traits for each rule kind

We could have defined `FormatRule`, `LintRule`, and `TypeRule` as separate traits. That would make the platform aware of every operation, which is the opposite of the goal. `Behavior<Op>` lets new operations be defined in new crates. The platform only needs to know that a rule is a `Behavior<Op>` for some `Operation`; it does not need to know what `Op` is.
