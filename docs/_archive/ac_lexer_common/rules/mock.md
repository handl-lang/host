**handl_lexer_common > rules > mock**

# Module: rules::mock

## Contents

**Structs**

- [`AtRule`](#atrule) - A simple rule that matches `@` and produces a [`MockKind::Custom`] token.
- [`BangRule`](#bangrule) - A stateful rule that matches `!` and produces a [`MockKind::Null`] token.
- [`HashRule`](#hashrule) - A simple rule that matches `#` and produces a [`MockKind::Custom`] token.
- [`RuleMock`](#rulemock) - Helpers for creating test rules and rule-related test fixtures.

---

## handl_lexer_common::rules::mock::AtRule

*Struct*

A simple rule that matches `@` and produces a [`MockKind::Custom`] token.

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, _config: &LexerConfig<MockKind, E>) -> Option<Result<Token<MockKind>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Clone**
  - `fn clone(self: &Self) -> AtRule`



## handl_lexer_common::rules::mock::BangRule

*Struct*

A stateful rule that matches `!` and produces a [`MockKind::Null`] token.

**Unit Struct**

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> BangRule`
- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, _config: &LexerConfig<MockKind, E>) -> Option<Result<Token<MockKind>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`



## handl_lexer_common::rules::mock::HashRule

*Struct*

A simple rule that matches `#` and produces a [`MockKind::Custom`] token.

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, _config: &LexerConfig<MockKind, E>) -> Option<Result<Token<MockKind>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Clone**
  - `fn clone(self: &Self) -> HashRule`



## handl_lexer_common::rules::mock::RuleMock

*Struct*

Helpers for creating test rules and rule-related test fixtures.

**Unit Struct**

**Methods:**

- `fn bang() -> BangRule` - Creates a rule that matches `!` and produces a [`MockKind::Null`] token.
- `fn hash() -> HashRule` - Creates a rule that matches `#` and produces a [`MockKind::Custom`] token.
- `fn at() -> AtRule` - Creates a rule that matches `@` and produces a [`MockKind::Custom`] token.



