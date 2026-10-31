+++
id = "ADR-0024"
title = "Agnostic parser engine: `handl-parser-common` and `handl-parser-canonical` on top"
status = "open"
priority = "medium"
+++

---
title: 'Agnostic parser engine: `handl-parser-common` and `handl-parser-canonical` on top'
type: adr
date: 2026-08-24
status: accepted
status_note: Accepted
---


# Agnostic parser engine: `handl-parser-common` and `handl-parser-canonical` on top

## Decision


Introduce a new crate `crates/parser-common` (package `handl-parser-common`) and refactor `crates/parser-canonical` (`handl-parser-canonical`) to sit on top of it.

### `handl-parser-common` — the generic parser runtime

`handl-parser-common` owns the parsing engine and knows nothing about handl expressions. It depends only on `handl-lexer` for tokens and `thiserror` for errors.

- **`ParseError`** — a span-aware error value. Carries a message and an optional `Span`.
- **`Rule<K, N>`** — one token kind's behavior in the grammar.
  - `left_bp` and `right_bp` for Pratt precedence.
  - `nud` (null denotation): parse a token with no left context.
  - `led` (left denotation): parse a token with an already-parsed left operand.
- **`Grammar<K, N>` trait** and **`SimpleGrammar<K, N>`** — the factory configuration.
  - `rules`: mapping from token kind to `Rule`.
  - `eof`: an optional end-of-input kind to verify.
  - `parse_root`: start a parse using the `ParserContext`.
- **`ParserContext<K, N>`** — the mutable parser state and primitive combinators.
  - `peek`, `advance`, `is_at_end`, `match_kind`, `expect` for recursive descent.
  - `expression(min_bp)` for Pratt operator-precedence parsing.
- **`Parser`** — the runtime.
  - `parse(&self, tokens: &[Token<K>], source: &str) -> Result<Ast<N>, ParseError>`
  - `Ast { source, root }` is generic over `N`, so a dialect can use `Node<ExprKind, V, Span>` from `handl-ast-common` or any other node type.

```rust
// simplified sketch
pub struct Rule<K, N> {
    pub left_bp: u32,
    pub right_bp: Option<u32>,
    pub nud: Option<Box<dyn Fn(&Token<K>, &mut ParserContext<K, N>) -> Result<N, ParseError>>>,
    pub led: Option<Box<dyn Fn(N, &Token<K>, &mut ParserContext<K, N>, u32) -> Result<N, ParseError>>>,
}

pub trait Grammar<K, N> {
    fn rule_for(&self, kind: &K) -> Option<&Rule<K, N>>;
    fn eof(&self) -> Option<K>;
    fn parse_root(&self, context: &mut ParserContext<K, N>) -> Result<N, ParseError>;
}
```

### `handl-parser-canonical` — the handl grammar on top

`handl-parser-canonical` stops being a hand-rolled state machine and becomes a `Grammar` definition over the existing `CanonicalKind` token enum.

- Define `Rule`s for `CanonicalKind::String`, `CanonicalKind::Number`, `CanonicalKind::Bool`, `CanonicalKind::Null`, `CanonicalKind::Identifier`, `CanonicalKind::Handle`, `CanonicalKind::CallOpen`, `CanonicalKind::BracketOpen`, `CanonicalKind::BlockOpen`, and the delimiters/comma/colon/as operators.
- The recursive structure (calls, arrays, objects, handler clauses) is built using the `ParserContext` combinators and `nud`/`led` rules rather than nested `ParserState` methods.
- It continues to produce `Expr<NonFunctionValue>` from `handl-ast`.

### Other crate impacts

- `handl-parser` keeps the `Parser` trait as the public boundary for now, but may eventually become a thin re-export over `handl-parser-common` or be removed.
- `handl-ast` and `handl-runtime` are not touched; they stay downstream of the parser.


## Why

`handl-ast-common` already gives us a generic, language-agnostic AST toolkit: `Node<K, V, M>`, pluggable `NodeMetadata`, and factory traits. `handl-lexer` gives us a generic token stream (`Token<K>` for any `TokenKind`). The parser side, however, has not been given the same treatment:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Composability.** New dialects assemble a `Grammar` from `Rule`s instead of copying `ParserState`.
- **Configurability.** The factory (`SimpleGrammar`) lets callers plug in rules without changing the parser engine.
- **Extensibility.** Adding a new operator or prefix form is adding one `Rule` entry; no new parser crate.
- **Decoupling.** `handl-parser-common` has no handl semantics. It is reusable for any token stream and node type.
- **Pratt + recursive descent.** The same engine supports both: `expression` for operator precedence, `parse_root` + `match_kind`/`expect` for statements and calls.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0024-parser-common.md`](../design/0024-parser-common.md).
