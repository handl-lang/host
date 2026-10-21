# Explanations

Explanations help you understand _why_ the platform works the way it
does. They are background reading, not step-by-step instructions — and
they are ordered the way the platform is layered: foundation first,
surface last.

## The kernel

- [What is handl?](./what-is-handl) — the platform's purpose and the
  publication boundary.
- [The arrow calculus kernel](./arrow-calculus-kernel) — why the
  six-combinator set is closed.
- [Graded algebraic theory](./graded-algebraic-theory) — the theoretical
  frame behind the platform.
- [Graded Modal Sanada Arrow Calculus](./graded-modal-sanada-arrow-calculus)
  — arrows + grades + Sanada's handlers, the canonical execution
  calculus.

## The engine layer

- [Language as data](./language-as-data) — a `Language` is a value
  assembled from YAML fragments; engines dispatch on data.
- [Runtime](./runtime) — how a host evaluates programs.

## The platform

- [Platform architecture](./platform-architecture) — the repo layers and
  how they compose.
- [The composable host](./composable-host) — why the platform is built
  around a single plugin model.
- [Plugins, rules, and operations](./plugins-rules-operations) — how
  `Plugin`, `RuleAcceptor`, `Operation`, and `Behavior` fit together.
- [Kits and manifests](./kits-and-manifests) — how kits curate plugins
  and defaults.
- [Options and configuration](./options-and-configuration) — how options
  are defined, resolved, and used.
- [Security and trust](./security-and-trust) — threat model and
  recommended practices.

## Papers and background

The papers live in the product repos; these are vendored copies:

- [Platform overview](../overview) — the plugin/host model at a glance.
- [Arrow-First Language Kernel](../PAPER) — the kernel paper.
- [Staged Arrow Engines](../SAE) — stackless spatial execution for the
  kernel's resolvable fragment.
- [Arrow support vs arrow-native](../comparison) — how handl compares to
  Haskell and other arrow-capable languages.
- [Cross-repo ADR index](../../adrs/) — every public decision
  record, aggregated.

## Design notes

Older design notes, merged here from the retired `explanations/`
directory:

- [Library API design principles](./ac-lib-api-design-principles)
- [Design principles](./design-principles)
- [Name explanation](./lang-name)
- [handl and other polyglot runtimes](./polyglot-runtimes)

## When to use another section

- If you are new to the platform, start with
  [Tutorials](../tutorials/setup).
- For practical instructions, see [How-to guides](../how-to/).
- For function and API listings, see the [Reference](../reference/).
- For the research stack, see [graphopt](../../graphopt/).
