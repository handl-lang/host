**handl_lexer**

# Module: handl_lexer

## Contents

**Modules**

- [`config`](#config) - Configuration for the lexing engine.
- [`error`](#error)
- [`lexer`](#lexer) - The [`Lexer`] trait — converts expression text into a token stream.
- [`primitives`](#primitives) - Low-level tokenization primitives.
- [`rule`](#rule) - Lexing rules — composable, type-safe building blocks for tokenization.
- [`span`](#span)
- [`token`](#token)

---

## Module: config

Configuration for the lexing engine.

[`LexerConfig`] holds three composable pieces:

- **Keywords** — identifier text mapped to a `TokenKind` (e.g. `true` → `Bool(true)`)
- **Delimiters** — single characters mapped to a `TokenKind` (e.g. `(` → `CallOpen`)
- **Rules** — ordered lexing functions that try to produce a token from the current
  position in the input

Configs can be merged with [`LexerConfig::merge`], enabling composition of dialects.
A dialect crate (e.g. `handl-lexer-canonical`) provides a preset function that populates
the config with the dialect's keywords, delimiters, and rules.



## Module: error



## Module: lexer

The [`Lexer`] trait — converts expression text into a token stream.



## Module: primitives

Low-level tokenization primitives.

Each function consumes characters from a `CharIndices` peekable
iterator and produces a raw value + span. These are reusable building
blocks for writing [`crate::LexRule`] functions — the rule functions
use a [`crate::KindFactory`] to convert raw values to dialect-specific
token kinds.



## Module: rule

Lexing rules — composable, type-safe building blocks for tokenization.

Each rule implements [`LexRule`] and tries to produce a token from the
current position in the input. Rules are tried in order by [`LexerConfig`];
the first that returns `Some` wins.

Built-in rules (`StringRule`, `NumberRule`, etc.) live in `handl-lexer-common`.
Custom rules can be defined as structs with state, or wrapped from function
pointers using [`FnRule`].



## Module: span



## Module: token



