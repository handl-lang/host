# handl_lexer_common

# handl-lexer-common

Shared test infrastructure and built-in rule kinds for the lexer crates.

Provides:
- **Token mocks**: [`MockKind`], [`MockFactory`] (configurable), [`MockLexer`].
- **Rule kinds**: [`StringRule`], [`NumberRule`], [`DelimiterRule`],
  [`IdentRule`], [`KeywordRule`], [`OperatorRule`], [`LineCommentRule`],
  [`BlockCommentRule`].
- **Rule mocks**: [`RuleMock`], [`BangRule`], [`HashRule`], [`AtRule`].
- **Config builders**: [`empty_config`], [`test_config`].

Depends on `handl-lexer` for core types (`LexRule`, `LexerConfig`, `Token`, etc.).

## Modules

### [`handl_lexer_common`](handl_lexer_common.md)

*2 modules*

### [`rules`](rules.md)

*10 modules*

### [`rules::block_comment`](rules/block_comment.md)

*1 struct*

### [`rules::config`](rules/config.md)

*2 functions*

### [`rules::delimiter`](rules/delimiter.md)

*1 struct*

### [`rules::ident`](rules/ident.md)

*1 struct*

### [`rules::keyword`](rules/keyword.md)

*1 struct*

### [`rules::line_comment`](rules/line_comment.md)

*1 struct*

### [`rules::mock`](rules/mock.md)

*4 structs*

### [`rules::number`](rules/number.md)

*1 struct*

### [`rules::operator`](rules/operator.md)

*1 struct*

### [`rules::string`](rules/string.md)

*1 struct*

### [`tokens`](tokens.md)

*3 modules*

### [`tokens::factory`](tokens/factory.md)

*1 struct*

### [`tokens::kind`](tokens/kind.md)

*1 enum*

### [`tokens::mock_lexer`](tokens/mock_lexer.md)

*1 struct*

