**handl_parser_common > grammar**

# Module: grammar

## Contents

**Structs**

- [`Rule`](#rule)
- [`SimpleGrammar`](#simplegrammar)

**Traits**

- [`Grammar`](#grammar)

---

## handl_parser_common::grammar::Grammar

*Trait*

**Methods:**

- `rule_for`
- `eof`
- `parse_root`



## handl_parser_common::grammar::Rule

*Struct*

**Generic Parameters:**
- K
- N
- E

**Fields:**
- `left_bp: u32`
- `right_bp: Option<u32>`
- `nud: Option<std::sync::Arc<dyn Fn>>`
- `led: Option<std::sync::Arc<dyn Fn>>`

**Methods:**

- `fn new(left_bp: u32) -> Self`
- `fn with_nud<F>(self: Self, f: F) -> Self`
- `fn with_led<F>(self: Self, f: F) -> Self`
- `fn with_right_bp(self: Self, right_bp: u32) -> Self`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Self`



## handl_parser_common::grammar::SimpleGrammar

*Struct*

**Generic Parameters:**
- K
- N
- E

**Fields:**
- `rules: std::collections::HashMap<std::mem::Discriminant<K>, Rule<K, N, E>>`
- `eof: Option<K>`
- `parse_root: std::sync::Arc<dyn Fn>`

**Methods:**

- `fn new() -> Self`
- `fn rule(self: Self, kind: K, rule: Rule<K, N, E>) -> Self`
- `fn eof(self: Self, kind: K) -> Self`
- `fn parse_root<F>(self: Self, f: F) -> Self`

**Trait Implementations:**

- **Grammar**
  - `fn rule_for(self: &Self, kind: &K) -> Option<Rule<K, N, E>>`
  - `fn eof(self: &Self) -> Option<K>`
  - `fn parse_root(self: &Self, context: & mut ParserContext<K, N, E>) -> Result<N, E>`



