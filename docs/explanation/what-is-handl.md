# What is handl?

handl is an expression language built on a composable, plugin-driven platform.

The language is a tiny kernel: literals, paths, function calls, and values. It has no built-in I/O, no control flow, and no standard library. The host defines everything else by loading plugins that contribute syntax, operations, and runtime behavior.

The platform is an empty `Host`. Load a kit of plugins, and the same host becomes a CLI, a formatter, an LSP server, a linter, a typechecker, or an interpreter. The host does not know which tool it is running. It only loads plugins, resolves their dependencies, and executes the operations they contribute.

This is the point: one language kernel, one host, and as many tools as the plugins allow.

## A language of data

handl programs are expressions that transform JSON-like data. Values, literals, and function calls all work with the same six JSON types. The language itself has no side effects, no built-in I/O, and no fixed syntax beyond a small kernel. Anything a program can do is provided by the host.

## A platform of plugins

The handl platform is not a monolithic compiler or runtime. It is a `Host` that loads `Plugin`s, resolves their dependencies, and assembles a language pipeline from rules:

- A parser is a `RuleAcceptor` for `Behavior<Parser>`.
- A formatter is a `RuleAcceptor` for `Behavior<Format>`.
- A target is a plugin that registers a runtime backend.
- A library is a plugin that registers more plugins.

Every plugin is the same kind of thing. The host is an empty composer. This means the same platform can become a CLI, an LSP server, a test runner, a formatter, or an embedded evaluator just by loading different kits of plugins.

## Why this matters

- **Composability over feature flags.** You do not enable a feature in a central compiler. You load the plugin that provides it and leave out the ones you do not need.
- **Host-defined capabilities.** `fs`, `env`, `http`, and other effects live in modules contributed by library and target plugins. The same script can run with a real OS backend, an in-memory stub, or a browser implementation depending on the host.
- **Testability.** Because the language has no side effects, a handl expression is a pure function from data to data. The parts that touch the world are explicit and swappable.

## Where handl fits

handl is a good fit for:

- Data processing and transformation scripts.
- Configuration, policy, or business logic that must run in more than one environment.
- Embedding a small expression language inside a larger application.

It is not a general-purpose systems language. It deliberately leaves I/O, concurrency, and platform details to the host, so it can be shaped to many different tools without changing the language core.
