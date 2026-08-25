# handl_lexer_canonical

# handl-lexer-canonical

Canonical lexer for handl expressions.

Implements [`Lexer`] using the canonical handl dialect syntax:
- `"..."` and `'...'` for strings (with backslash escapes)
- `42`, `1.5`, `-3` for numbers
- `true` / `false` for booleans, `null` for null
- `handle` and `as` as keywords
- `(` `)` `{` `}` `[` `]` `,` `:` `.` as delimiters
- `// line comments` and `/* block comments */`

Dots are separate `Dot` tokens; `math.sqrt` lexes as `math`, `.`, `sqrt`.

The canonical config is available as [`canonical_config()`]; custom
dialects can start from it and modify keywords, delimiters, and rules
using [`LexerConfig`]'s builder methods.

## Modules

### [`handl_lexer_canonical`](handl_lexer_canonical.md)

*3 modules*

### [`config`](config.md)

*1 function, 2 structs*

### [`kind`](kind.md)

*1 enum, 1 struct*

### [`lexer`](lexer.md)

*2 structs*

### [`lexer::CanonicalLexer`](lexer/CanonicalLexer.md)

*1 function*

