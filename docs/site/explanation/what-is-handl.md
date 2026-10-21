# What is handl?

handl is a **platform for building languages** — a closed arrow-calculus
kernel, a data-driven engine layer, and a composable plugin host.

The published pieces today are the machinery, not the language. The handl
surface language (its syntax, its standard library, and the `hnl`
toolchain) is still in development and is deliberately not documented
here.

## Three ideas, one stack

**The kernel is a closed arrow calculus.** All computation is an arrow
`A ~> B` built from six combinators — `arr`, `>>>`, `first`, `choice`,
`loop`, `app`. Everything is an object in one world (`Obj : sym()`);
types, grades, proofs, and capabilities are ordinary objects, and there
is no separate lambda, closure, or mutable-environment machinery. See
[The arrow calculus kernel](./arrow-calculus-kernel) and
[ADR-0106](https://github.com/handl-lang/host/blob/project/adrs/0106-object-arrow-kernel.md).

**A language is data.** A `Language` value is assembled from YAML
fragments — tokens, grammar, eval schemes, types, AST shapes. Engines for
lexing, parsing, transforming, and executing are generic: they dispatch
on the `Language` data, never on hardcoded constructs. A new language is
a data change, not an engine recompile. See [Language as data](./language-as-data).

**The host is a composer.** An empty `Host` loads `Plugin`s, resolves
their dependencies, runs `contribute` to a fixed point, and dispatches
capabilities. A parser is a `RuleAcceptor` for `Behavior<Parser>`; a
formatter accepts `Behavior<Format>`; a target contributes evaluation or
compilation rules. The host does not know which tool it becomes. See
[The composable host](./composable-host).

## Why this matters

- **Composability over feature flags.** You do not enable a feature in a
  central compiler — you load the plugin that provides it, and leave out
  the ones you do not need.
- **Host-defined capabilities.** I/O and platform services live in
  modules contributed by library and target plugins, so the same program
  can run against a real OS backend, an in-memory stub, or a browser
  implementation depending on the host.
- **A small, honest core.** Because the calculus is closed and the
  language is data, every consumer — interpreter, typechecker,
  pretty-printer, serializer — covers a fixed, tiny vocabulary.

## Where the platform fits

The platform is a good fit for:

- Building an embeddable expression language inside a larger application.
- Assembling DSLs and language variants from shared machinery without
  forking a compiler.
- Tools that must agree on one language definition — a formatter, a
  linter, an LSP server, and an interpreter can all consume the same
  `Language` data.

It is not a general-purpose systems language. It deliberately leaves I/O,
concurrency, and platform details to the host, so the same machinery can
be shaped into many different tools.
