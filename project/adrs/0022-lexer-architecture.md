# Lexer Architecture: Self-Hosted Streaming with Runtime Config

## Context

The `handl-lexer` trait and `handl-lexer-canonical` implementation need to support:

1. **Streaming** — token-by-token iteration for pipelines like audio → STT → tokens → parser → VM, where input arrives incrementally.
2. **Runtime configurability** — adding/modifying keywords and delimiters without recompiling, enabling dialects and custom frontends.

No existing Rust crate provides both:

| Crate | Streaming | Runtime config | Notes |
|---|---|---|---|
| `logos` | Yes (iterator) | No (proc-macro, compile-time) | Fastest option, but token rules are fixed at compile time |
| `chumsky` | Experimental | Yes (combinators) | Parser combinator, not a lexer; heavy for tokenization |
| `nom` / `winnow` | Partial (`Incomplete`) | Yes | Parser combinators, not designed for lexing |
| `pest` / `lalrpop` | No | No (compile-time grammar) | Batch-only, compile-time |

## Decision

Keep the self-hosted `CanonicalLexer` (~200 lines). Add a streaming method to the `Lexer` trait alongside the existing batch `lex` method. Do not adopt `logos` or build a lexer framework.

### Rationale

- The canonical lexer already processes char-by-char, so yielding tokens incrementally is a natural extension — not a redesign.
- `LexerConfig` provides runtime keyword/delimiter configurability that `logos` cannot offer without a two-pass approach.
- The `Lexer` trait is the abstraction boundary: the parser consumes `Vec<Token>` regardless of whether tokens came from text, audio, or any other source. A future `Lexer<Input>` generalization decouples from `&str` when non-text modalities arrive.
- Building a lexer framework would be over-engineering at this scale. The trait + config + streaming method covers the design space.

### Streaming addition

```rust
trait Lexer {
    fn lex(&self, input: &str) -> Result<Vec<Token>, LexError>;
    fn lex_iter(&self, input: &str) -> impl Iterator<Item = Result<Token, LexError>>;
}
```

The batch method remains for simplicity. The streaming method enables incremental pipelines. Both share the same tokenization logic.

## Consequences

- No external lexer dependency — the crate stays dependency-free except `thiserror`.
- Streaming is opt-in; existing consumers using `lex()` are unaffected.
- Runtime dialects via `LexerConfig` continue to work in both batch and streaming modes.
- If raw lexing speed becomes a bottleneck, the canonical implementation can be swapped to a `logos`-backed DFA internally while preserving the trait and config — but this trades runtime configurability for speed and is deferred until measured.
