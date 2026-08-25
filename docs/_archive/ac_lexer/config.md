**handl_lexer > config**

# Module: config

## Contents

**Structs**

- [`LexerConfig`](#lexerconfig) - Configuration for the lexing engine.

---

## handl_lexer::config::LexerConfig

*Struct*

Configuration for the lexing engine.

Holds keywords, delimiters, and an ordered list of [`LexRule`] trait objects.
Start with an empty config ([`LexerConfig::new`]) or a dialect preset,
then customize with the builder methods.

# Examples

```ignore
use handl_lexer::{TokenKind, LexerConfig, StringRule, DefaultLexError};

let config = LexerConfig::new(MyFactory)
    .with_keyword("true", MyKind::Bool(true))
    .with_delimiter('(', MyKind::CallOpen)
    .with_rule(StringRule);
```

**Generic Parameters:**
- K
- E

**Fields:**
- `keywords: Vec<(String, K)>`
- `delimiters: Vec<(char, K)>`
- `factory: <K as >::Factory`
- `rules: Vec<Box<dyn LexRule>>`

**Methods:**

- `fn new(factory: <K as >::Factory) -> Self` - Creates an empty configuration with the given kind factory.
- `fn factory(self: &Self) -> &<K as >::Factory` - Returns the kind factory.
- `fn with_keyword(self: Self, text: &str, kind: K) -> Self` - Adds or replaces a keyword mapping.
- `fn without_keyword(self: Self, text: &str) -> Self` - Removes a keyword mapping.
- `fn lookup_keyword(self: &Self, text: &str) -> Option<&K>` - Looks up a keyword by identifier text.
- `fn with_delimiter(self: Self, ch: char, kind: K) -> Self` - Adds or replaces a delimiter mapping.
- `fn without_delimiter(self: Self, ch: char) -> Self` - Removes a delimiter mapping.
- `fn lookup_delimiter(self: &Self, ch: char) -> Option<&K>` - Looks up a delimiter by character.
- `fn with_rule<impl LexRule<K, E> + 'static>(self: Self, rule: impl Trait) -> Self` - Appends a lexing rule to the end of the rule list.
- `fn insert_rule<impl LexRule<K, E> + 'static>(self: Self, index: usize, rule: impl Trait) -> Self` - Inserts a lexing rule at the given index, shifting existing rules.
- `fn without_rule(self: Self, rule: &dyn LexRule) -> Self` - Removes all rules with the same name as the given rule.
- `fn without_rule_by_name(self: Self, name: &str) -> Self` - Removes all rules with the given name.
- `fn with_rules(self: Self, rules: Vec<Box<dyn LexRule>>) -> Self` - Replaces the entire rule list.
- `fn rules(self: &Self) -> &[Box<dyn LexRule>]` - Returns the ordered list of lexing rules.
- `fn merge(self: Self, other: LexerConfig<K, E>) -> Self` - Merges another config into this one.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Self`



