**handl_lexer_common > rules > line_comment**

# Module: rules::line_comment

## Contents

**Structs**

- [`LineCommentRule`](#linecommentrule) - Matches line comments using the factory's [`CommentFactory::is_line_comment_start`]

---

## handl_lexer_common::rules::line_comment::LineCommentRule

*Struct*

Matches line comments using the factory's [`CommentFactory::is_line_comment_start`]
and [`CommentFactory::lex_line_comment`].

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> LineCommentRule`
- **Clone**
  - `fn clone(self: &Self) -> LineCommentRule`



