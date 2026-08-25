# handl_parser_canonical

# handl-parser-canonical

Canonical recursive-descent parser for handl expressions.

Implements [`Parser`] using the canonical grammar:

```text
expr      := value | call | handle
value     := string | number | byte | bool | null
byte      := "0x" hex_digit{1,2}
call      := identifier "(" args? ")"
args      := expr ("," expr)*
handle    := "handle" "(" handler-clause ("," handler-clause)* ")"
handler   := string "(" params? ")" "as" identifier ":" expr
```

The parser consumes tokens from [`CanonicalLexer`] (or any
[`Lexer`](handl_lexer::Lexer) producing compatible tokens) and
produces an [`Expr`] tree ready for the VM.

## Modules

### [`handl_parser_canonical`](handl_parser_canonical.md)

*1 module*

### [`parser`](parser.md)

*1 enum, 1 function, 1 static, 2 structs*

