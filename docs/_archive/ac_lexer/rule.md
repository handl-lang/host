**handl_lexer > rule**

# Module: rule

## Contents

**Modules**

- [`fn_rule`](#fn_rule) - [`FnRule`] — a rule backed by a function pointer.

**Traits**

- [`LexRule`](#lexrule) - A lexing rule — inspects the current position and optionally produces a token.

---

## handl_lexer::rule::LexRule

*Trait*

A lexing rule — inspects the current position and optionally produces a token.

Returns `None` if the rule does not match (try the next rule),
`Some(Ok(token))` on success, or `Some(Err(_))` on a matched-but-failed
lex (e.g. unterminated string).

# Implementing a custom rule

```
use handl_lexer::{DefaultLexError, EofFactory, LexRule, LexerConfig, Span, Token, TokenKind};
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

// A stateful custom rule.
#[derive(Clone)]
struct HashRule;

impl LexRule<MyKind, DefaultLexError> for HashRule {
    fn try_lex(
        &self,
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

    fn name(&self) -> &'static str { "hash" }

    fn clone_box(&self) -> Box<dyn LexRule<MyKind, DefaultLexError>> {
        Box::new(self.clone())
    }
}
```

**Methods:**

- `try_lex`: Try to lex a token from the current position.
- `name`: A name for deduplication and debugging.
- `clone_box`: Clone into a boxed trait object.



## Module: fn_rule

[`FnRule`] — a rule backed by a function pointer.



