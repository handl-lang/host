**handl_lexer_common > rules > config**

# Module: rules::config

## Contents

**Functions**

- [`empty_config`](#empty_config) - Returns an empty `LexerConfig<MockKind, DefaultLexError>` with just the
- [`test_config`](#test_config) - Returns a `LexerConfig<MockKind, DefaultLexError>` with all built-in rules

---

## handl_lexer_common::rules::config::empty_config

*Function*

Returns an empty `LexerConfig<MockKind, DefaultLexError>` with just the
factory — no rules, no keywords, no delimiters. Use this when tests need
full control.

```rust
fn empty_config() -> handl_lexer::LexerConfig<crate::tokens::MockKind, handl_lexer::DefaultLexError>
```



## handl_lexer_common::rules::config::test_config

*Function*

Returns a `LexerConfig<MockKind, DefaultLexError>` with all built-in rules
and a few common keywords/delimiters pre-configured.

```rust
fn test_config() -> handl_lexer::LexerConfig<crate::tokens::MockKind, handl_lexer::DefaultLexError>
```



