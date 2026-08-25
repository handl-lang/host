**handl_lexer > rule > fn_rule**

# Module: rule::fn_rule

## Contents

**Structs**

- [`FnRule`](#fnrule) - A rule backed by a function pointer.

---

## handl_lexer::rule::fn_rule::FnRule

*Struct*

A rule backed by a function pointer.

Use this when you need a simple rule without defining a struct.
For stateful or composable rules, implement [`LexRule`] directly.

```
use handl_lexer::{DefaultLexError, EofFactory, FnRule, LexerConfig, TokenKind, LexError, Span, Token};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum MyKind { Hash(String), Eof }

impl fmt::Display for MyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { MyKind::Hash(s) => write!(f, "hash({s})"), MyKind::Eof => write!(f, "eof") }
    }
}

#[derive(Clone)]
struct MyFactory;
impl EofFactory<MyKind> for MyFactory {
    fn eof_kind(&self) -> MyKind { MyKind::Eof }
}

impl TokenKind for MyKind {
    type Factory = MyFactory;
    fn is_eof(&self) -> bool { matches!(self, MyKind::Eof) }
    fn is_error(&self) -> bool { false }
}

fn hash_rule(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    _config: &LexerConfig<MyKind, DefaultLexError>,
) -> Option<Result<Token<MyKind>, DefaultLexError>> {
    match chars.peek() {
        Some(&(_, '#')) => {
            let (start, _) = chars.next().unwrap();
            Some(Ok(Token::new(MyKind::Hash("hash".into()), Span::at(start))))
        }
        _ => None,
    }
}

let config = LexerConfig::new(MyFactory)
    .with_rule(FnRule::new("hash", hash_rule));
```

**Generic Parameters:**
- K
- E

**Fields:**
- `name: &'static str`
- `func: fn(...)`

**Methods:**

- `fn new(name: &'static str, func: fn(...)) -> Self` - Creates a named function-pointer rule.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Self`
- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`



