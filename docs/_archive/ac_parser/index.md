# handl_parser

# handl-parser

Parsing abstraction for handl expressions.

Defines the [`Parser`] trait and [`ParseError`] — the interface for
turning a token stream (from [`Lexer`](handl_lexer::Lexer)) into an
[`Expr`](handl_ast_canonical::Expr) AST. The canonical implementation lives in
`handl-parser-canonical`.

## Architectural rule

> **The parser consumes tokens and produces `Expr`.**

The parser is the bridge between the lexer and the VM. It depends on
`handl-lexer` (for `Token`/`TokenKind`) and `handl-ast-canonical` (for `Expr`),
but not on the VM. This keeps the parsing pipeline independent of
evaluation strategy.

## Modules

### [`handl_parser`](handl_parser.md)

*1 module, 1 trait*

### [`Parser`](Parser.md)

*1 function*

### [`error`](error.md)

*1 enum*

