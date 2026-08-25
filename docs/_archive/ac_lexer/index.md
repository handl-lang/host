# handl_lexer

# handl-lexer

Lexing abstraction for handl expressions.

Defines the [`Lexer`] trait, [`Token`], [`TokenKind`], [`Span`], and
[`LexError`] — the types needed to turn raw expression text into a
token stream. The canonical implementation lives in `handl-lexer-canonical`.

## Architectural rule

> **The lexer produces tokens; the parser consumes them.**

The lexer is deliberately separate from the parser so that tooling
(LSP, formatters, linters) can depend on tokens without pulling in
the full parser and AST.

## Modules

### [`handl_lexer`](handl_lexer.md)

*7 modules*

### [`config`](config.md)

*1 struct*

### [`config::LexerConfig`](config/LexerConfig.md)

*3 functions*

### [`error`](error.md)

*1 enum, 1 trait*

### [`lexer`](lexer.md)

*1 trait*

### [`lexer::Lexer`](lexer/Lexer.md)

*2 functions*

### [`primitives`](primitives.md)

*1 constant, 11 functions*

### [`rule`](rule.md)

*1 module, 1 trait*

### [`rule::fn_rule`](rule/fn_rule.md)

*1 struct*

### [`span`](span.md)

*1 struct*

### [`token`](token.md)

*1 struct, 10 traits*

### [`token::IdentFactory`](token/IdentFactory.md)

*1 function*

### [`token::OperatorFactory`](token/OperatorFactory.md)

*1 function*

