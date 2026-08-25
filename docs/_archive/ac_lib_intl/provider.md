**handl_lib_intl > provider**

# Module: provider

## Contents

**Structs**

- [`IntlProvider`](#intlprovider) - A shared ICU4X data provider backed by a blob.

---

## handl_lib_intl::provider::IntlProvider

*Struct*

A shared ICU4X data provider backed by a blob.

When set on `IntlModule`, all functions will use this provider
instead of the compiled CLDR data. This enables runtime data loading
with custom locale subsets for smaller binaries.

Create from a `.postcard` blob file:
```no_run
use handl_lib_intl::IntlModule;

let blob = std::fs::read("data.postcard").unwrap();
let module = IntlModule::with_blob_provider(blob);
```

The raw bytes are stored as `Arc<[u8]>` so that cloning `IntlProvider`
(e.g. when distributing it to each function struct in `with_provider`)
is a cheap refcount increment, not a deep copy.
A `BlobDataProvider` is created on each function call from the stored
bytes, which requires copying into a `Box<[u8]>` (ICU4X API requirement).

**Fields:**
- `blob: std::sync::Arc<[u8]>`

**Methods:**

- `fn from_blob(blob: Vec<u8>) -> Self` - Create a provider from a blob of ICU4X postcard data.
- `fn as_blob_data_provider(self: &Self) -> Result<BlobDataProvider, icu_provider::DataError>` - Create a `BlobDataProvider` from the stored bytes.
- `fn as_bytes(self: &Self) -> &[u8]` - Returns the raw blob bytes.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IntlProvider`



