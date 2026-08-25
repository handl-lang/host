**handl_lib_intl > module > IntlModule**

# Module: module::IntlModule

## Contents

**Functions**

- [`with_blob_provider`](#with_blob_provider) - Create a new `IntlModule` from a blob of ICU4X postcard data.
- [`with_provider`](#with_provider) - Create a new `IntlModule` using a runtime-loaded data blob.

---

## handl_lib_intl::module::IntlModule::with_blob_provider

*Function*

Create a new `IntlModule` from a blob of ICU4X postcard data.

The blob is not validated until the first function call.

```rust
fn with_blob_provider(blob: Vec<u8>) -> Self
```



## handl_lib_intl::module::IntlModule::with_provider

*Function*

Create a new `IntlModule` using a runtime-loaded data blob.

The blob must be in ICU4X postcard format (`.postcard`).
Generate one with `icu4x-datagen`:
```sh
icu4x-datagen --keys all --locales en,de,ja --format blob --out data.postcard
```

```rust
fn with_provider(provider: IntlProvider) -> Self
```



