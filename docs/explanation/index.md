# Explanations

Explanations help you understand *why* handl works the way it does. They are background reading, not step-by-step instructions.

## The language

- [What is handl?](./what-is-handl) — the language's purpose and philosophy.
- [Language overview](./language-overview) — values, expressions, functions, and modules.
- [Runtime](./runtime) — how handl expressions are executed.

## The platform

- [Platform architecture](./platform-architecture) — the crate layers and how they compose.
- [The composable host](./composable-host) — why the platform is built around a single plugin model.
- [Plugins, rules, and operations](./plugins-rules-operations) — how `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` fit together.
- [Kits and manifests](./kits-and-manifests) — how kits curate plugins and defaults.
- [Options and configuration](./options-and-configuration) — how options are defined, resolved, and used.
- [Security and trust](./security-and-trust) — threat model and recommended practices.

## When to use another section

- If you are new to handl, start with [Tutorials](../tutorials/getting-started).
- For practical instructions, see [How-to guides](../how-to/).
- For function and API listings, see the [Reference](../reference/).
