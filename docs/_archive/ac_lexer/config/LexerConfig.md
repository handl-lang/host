**handl_lexer > config > LexerConfig**

# Module: config::LexerConfig

## Contents

**Functions**

- [`merge`](#merge) - Merges another config into this one.
- [`new`](#new) - Creates an empty configuration with the given kind factory.
- [`without_rule`](#without_rule) - Removes all rules with the same name as the given rule.

---

## handl_lexer::config::LexerConfig::merge

*Function*

Merges another config into this one.

- **Keywords**: `other`'s keywords are added or replace existing ones.
- **Delimiters**: `other`'s delimiters are added or replace existing ones.
- **Rules**: `other`'s rules are appended (duplicates by name are skipped).

```rust
fn merge(self: Self, other: LexerConfig<K, E>) -> Self
```



## handl_lexer::config::LexerConfig::new

*Function*

Creates an empty configuration with the given kind factory.

```rust
fn new(factory: <K as >::Factory) -> Self
```



## handl_lexer::config::LexerConfig::without_rule

*Function*

Removes all rules with the same name as the given rule.

```rust
fn without_rule(self: Self, rule: &dyn LexRule) -> Self
```



