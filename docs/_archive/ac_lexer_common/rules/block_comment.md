**handl_lexer_common > rules > block_comment**

# Module: rules::block_comment

## Contents

**Structs**

- [`BlockCommentRule`](#blockcommentrule) - Matches block comments using the factory's [`CommentFactory::is_block_comment_start`]

---

## handl_lexer_common::rules::block_comment::BlockCommentRule

*Struct*

Matches block comments using the factory's [`CommentFactory::is_block_comment_start`]
and [`CommentFactory::lex_block_comment`].

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> BlockCommentRule`
- **Clone**
  - `fn clone(self: &Self) -> BlockCommentRule`



