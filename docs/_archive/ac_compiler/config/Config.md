**handl_compiler > config > Config**

# Module: config::Config

## Contents

**Functions**

- [`extension`](#extension) - Borrow a typed extension previously added via [`Config::with_extension`].
- [`with_extension`](#with_extension) - Add a typed extension. Enables custom passes to receive config

---

## handl_compiler::config::Config::extension

*Function*

Borrow a typed extension previously added via [`Config::with_extension`].

```rust
fn extension<T>(self: &Self) -> Option<&T>
```



## handl_compiler::config::Config::with_extension

*Function*

Add a typed extension. Enables custom passes to receive config
without modifying `Config`.

```rust
fn with_extension<T>(self: Self, ext: T) -> Self
```



