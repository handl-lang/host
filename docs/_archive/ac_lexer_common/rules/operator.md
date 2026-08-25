**handl_lexer_common > rules > operator**

# Module: rules::operator

## Contents

**Structs**

- [`OperatorRule`](#operatorrule) - Matches operators using the factory's [`OperatorFactory::lex_operator`].

---

## handl_lexer_common::rules::operator::OperatorRule

*Struct*

Matches operators using the factory's [`OperatorFactory::lex_operator`].

**Unit Struct**

**Trait Implementations:**

- **Default**
  - `fn default() -> OperatorRule`
- **Clone**
  - `fn clone(self: &Self) -> OperatorRule`
- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`



