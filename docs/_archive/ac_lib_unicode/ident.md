**handl_lib_unicode > ident**

# Module: ident

## Contents

**Functions**

- [`is_identifier`](#is_identifier) - Returns `true` if `value` is a well-formed identifier.
- [`is_identifier_continue`](#is_identifier_continue) - Returns `true` if `ch` is a valid non-first character of an identifier.
- [`is_identifier_start`](#is_identifier_start) - Returns `true` if `ch` is a valid first character of an identifier.

---

## handl_lib_unicode::ident::is_identifier

*Function*

Returns `true` if `value` is a well-formed identifier.

```rust
fn is_identifier(value: &str) -> bool
```



## handl_lib_unicode::ident::is_identifier_continue

*Function*

Returns `true` if `ch` is a valid non-first character of an identifier.

```rust
fn is_identifier_continue(ch: char) -> bool
```



## handl_lib_unicode::ident::is_identifier_start

*Function*

Returns `true` if `ch` is a valid first character of an identifier.

```rust
fn is_identifier_start(ch: char) -> bool
```



