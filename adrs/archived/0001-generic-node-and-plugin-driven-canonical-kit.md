---
title: Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit
type: adr
date: 2026-08-29
status: accepted
status_note: Accepted
---

# ADR-0001: Generic Node, Trait-Based Capabilities, and Plugin-Driven Canonical Kit

## Context

The Handl platform currently defines concrete canonical implementations in three dedicated crates:

- `handl-ast-canonical` — concrete expression, statement, pattern, type, and field nodes.
- `handl-lexer-canonical` — concrete token kind `CanonicalKind` and the canonical lexer.
- `handl-parser-canonical` — concrete recursive-descent parser with a hard-coded `CanonicalGrammar`.

These crates are widely depended upon by plugins, libraries, hosts, and the root workspace package. They were a pragmatic starting point, but they make the canonical language a monolith rather than a composition of independent features.

We want the canonical language to be a curated kit (`kit-canonical`) assembled from small, composable plugins. The same capability model should apply to AST nodes, lexer tokens, grammar rules, and eventually runtime value types.

## Decision

We will:

1. Define **one generic `Node` type** in `handl-ast`. Concrete node shapes will be provided by plugins. A language only includes the node shapes it loads.
2. Model **capabilities as Rust traits**. A node, token, grammar rule, or value type implements zero or more capability traits (`Expr`, `Stmt`, `Pattern`, `TokenKind`, `GrammarRule`, `Addable`, `Comparable`, etc.).
3. Make **tokens, node shapes, and grammar rules pluggable at the smallest useful unit**. Each canonical token, AST node, and grammar rule becomes a tiny plugin that the kit composes.
4. Turn `kit-canonical` into a **curated plugin bundle**, not the implementation itself.

## Consequences

### Base crates become pure abstractions

| crate          | new responsibility                                              |
| -------------- | --------------------------------------------------------------- |
| `handl-ast`    | defines `Node` and per-shape node capabilities                  |
| `handl-lexer`  | defines `Token`, `Lexer`, and `TokenKind` capability trait      |
| `handl-parser` | defines `Parser`, `Grammar`, and `GrammarRule` capability trait |

### Canonical implementation becomes plugins

Existing `*-canonical` crates will be split into the smallest composable plugins, for example:

| domain              | example plugins                                                                                                                             |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| AST node shapes     | `plugin-ast-call`, `plugin-ast-index`, `plugin-ast-pipe`, `plugin-ast-assign`, `plugin-ast-let`, `plugin-ast-match`                         |
| Token kinds         | `plugin-token-number`, `plugin-token-string`, `plugin-token-identifier`, `plugin-token-comment`                                             |
| Grammar rules       | `plugin-grammar-call`, `plugin-grammar-index`, `plugin-grammar-pipe`, `plugin-grammar-assign`, `plugin-grammar-let`, `plugin-grammar-match` |
| Value type families | `plugin-value-integer`, `plugin-value-string`, `plugin-value-decimal`                                                                       |

`kit-canonical` will become a manifest that registers the curated set of these tiny plugins.

### Value types use the same model

Runtime value types will also be assembled from capabilities (`Addable`, `Comparable`, `Hashable`, `Serializable`, etc.). Plugins can introduce new value families, and target-specific representations (native, JS, WASM) are selected by the compiler.

## Migration plan

1. **Genericize `handl-ast`**: replace `Expr<NonFunctionValue>` with a single `Node` type and per-shape node traits.
2. **Genericize `handl-lexer`**: make `TokenKind` a plugin trait; remove concrete `CanonicalKind` from the base crate.
3. **Genericize `handl-parser`**: make `Grammar` composable from `GrammarRule` plugins.
4. **Create canonical plugins**: extract canonical nodes, token kinds, and grammar rules into per-shape plugins such as `plugin-ast-*`, `plugin-token-*`, `plugin-grammar-*`, and `plugin-value-*`.
5. **Rewrite `kit-canonical`**: make it a host that loads the canonical plugin manifest.
6. **Migrate dependents**: update plugins, libraries, and hosts to depend on the generic base crates or on `kit-canonical`.
7. **Delete `ast-canonical`, `lexer-canonical`, and `parser-canonical`** once the kit and all dependents are moved.

## Alternatives considered

- **Keep `*-canonical` crates**: rejected because they make the canonical language a monolith that cannot be extended or subset by other kits.
- **Use runtime capability descriptors instead of Rust traits**: rejected because we want zero-cost static dispatch and trait-based type safety.
- **One crate per canonical feature instead of tiny plugins**: rejected because it breaks the plugin boundary and makes the kit less composable.

## Implementation status

- `handl-ast` now provides the generic `Node` type and per-shape node capability traits; concrete node shapes live in `plugin-ast-*` crates.
- `handl-lexer` and `handl-parser` have been genericized to `Token`/`Lexer` and `Parser`/`Grammar` abstractions with `TokenKind` and `GrammarRule` capability traits.
- `kit-canonical` has been rewritten as a curated plugin manifest rather than a concrete implementation crate.
- `handl-ast-canonical`, `handl-lexer-canonical`, and `handl-parser-canonical` crates have been removed.

## Related work

- Composable value types design
- `docs/explanations/design-principles.md`
- `kit-canonical` crate manifest
