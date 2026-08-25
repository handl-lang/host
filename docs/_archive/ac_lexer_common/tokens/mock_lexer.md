**handl_lexer_common > tokens > mock_lexer**

# Module: tokens::mock_lexer

## Contents

**Structs**

- [`MockLexer`](#mocklexer) - A mock lexer that returns a predefined token list or error.

---

## handl_lexer_common::tokens::mock_lexer::MockLexer

*Struct*

A mock lexer that returns a predefined token list or error.

**Generic Parameters:**
- E

**Fields:**
- `tokens: Result<Vec<handl_lexer::Token<super::MockKind>>, E>`

**Methods:**

- `fn ok(tokens: Vec<Token<MockKind>>) -> Self`
- `fn err(e: E) -> Self`

**Trait Implementations:**

- **Lexer**
  - `fn lex(self: &Self, _input: &str) -> Result<Vec<Token<MockKind>>, E>`



