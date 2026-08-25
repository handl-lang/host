**handl_lexer_canonical > kind**

# Module: kind

## Contents

**Structs**

- [`CanonicalFactory`](#canonicalfactory) - Factory for constructing [`CanonicalKind`] tokens from raw lexed content.

**Enums**

- [`CanonicalKind`](#canonicalkind) - The canonical handl expression token kinds.

---

## handl_lexer_canonical::kind::CanonicalFactory

*Struct*

Factory for constructing [`CanonicalKind`] tokens from raw lexed content.

**Unit Struct**

**Trait Implementations:**

- **IdentFactory**
  - `fn ident_kind(self: &Self, text: String) -> CanonicalKind`
  - `fn is_ident_start(self: &Self, ch: char) -> bool`
  - `fn is_ident_continue(self: &Self, ch: char) -> bool`
- **NumberFactory**
  - `fn number_kind(self: &Self, value: f64) -> CanonicalKind`
- **StringFactory**
  - `fn string_kind(self: &Self, value: String) -> CanonicalKind`
- **EofFactory**
  - `fn eof_kind(self: &Self) -> CanonicalKind`
- **Default**
  - `fn default() -> CanonicalFactory`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalFactory`
- **CommentFactory**
  - `fn comment_kind(self: &Self, text: String) -> CanonicalKind`



## handl_lexer_canonical::kind::CanonicalKind

*Enum*

The canonical handl expression token kinds.

This is the dialect-specific enum implementing [`TokenKind`] for the
canonical handl syntax. Other dialects can define their own enums and
reuse the generic lexing infrastructure from `handl-lexer`.

**Variants:**
- `Identifier(String)` - An identifier — function name or variable name
- `String(String)` - A string literal (already unescaped, without quotes).
- `Number(f64)` - A number literal.
- `Bool(bool)` - A boolean literal.
- `Byte(u8)` - A byte literal (`0xNN`).
- `Null` - The null literal.
- `Handle` - The `handle` keyword.
- `As` - The `as` keyword (used in handler clauses).
- `Is` - The `is` keyword (interrogative mood).
- `Must` - The `must` keyword (normative mood).
- `May` - The `may` keyword (potential mood).
- `Not` - The `not` keyword (negative mood or logical negation).
- `When` - The `when` keyword (conditional mood).
- `Else` - The `else` keyword (conditional mood fallback).
- `Match` - The `match` keyword (pattern matching expression).
- `CallOpen` - Call-open delimiter — `(` in canonical.
- `CallClose` - Call-close delimiter — `)` in canonical.
- `BlockOpen` - Block-open delimiter — `{` in canonical.
- `BlockClose` - Block-close delimiter — `}` in canonical.
- `BracketOpen` - Bracket-open delimiter — `[` in canonical.
- `BracketClose` - Bracket-close delimiter — `]` in canonical.
- `Comma` - Argument separator — `,` in canonical.
- `Colon` - Clause separator — `:` in handler clauses.
- `Arrow` - Arrow — `->` (first-class function body introducer).
- `Pipe` - Pipe forward — `|>` (left-to-right value threading).
- `BackwardPipe` - Pipe backward — `<|` (right-to-left value threading).
- `Flow` - Flow / forward composition — `>>>` (arrow combinator).
- `Compose` - Composition / backward composition — `<<<` (arrow combinator).
- `Fanout` - Fanout — `&&&` (arrow combinator).
- `Parallel` - Parallel / fanin — `||||` (arrow combinator).
- `Question` - Ternary / optional — `?`.
- `Range` - Range — `..`.
- `Spread` - Spread — `...`.
- `LessThan` - Less-than — `<` (generic parameter list opener).
- `GreaterThan` - Greater-than — `>` (generic parameter list closer).
- `Equal` - Equal — `=` (assignment).
- `Dot` - Dot — `.` (field access).
- `Comment(String)` - A comment — `// line` or `/* block */`. Filtered out of the
- `Eof` - End of input.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &CanonicalKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalKind`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **CommentTokenKind**
  - `fn is_comment(self: &Self) -> bool`
- **TokenKind**
  - `fn is_eof(self: &Self) -> bool`
  - `fn is_error(self: &Self) -> bool`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`



