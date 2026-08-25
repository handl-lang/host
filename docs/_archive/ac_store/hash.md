**handl_store > hash**

# Module: hash

## Contents

**Structs**

- [`DefinitionHash`](#definitionhash) - A content hash identifying a definition.

---

## handl_store::hash::DefinitionHash

*Struct*

A content hash identifying a definition.

Uses SHA-256, hex-encoded. The hash is computed from the canonical
JSON representation of a definition — sorted keys, no whitespace.

Two definitions with the same canonical form produce the same hash,
regardless of formatting or key order in the source.

**Tuple Struct**: `(String)`

**Methods:**

- `fn from_canonical_string(canonical: &str) -> Self` - Computes a hash from a canonical JSON string.
- `fn from_json(value: &serde_json::Value) -> Self` - Computes a hash from a JSON value (canonicalized first).
- `fn from_hex<impl Into<String>>(hex: impl Trait) -> Self` - Creates a `DefinitionHash` from a raw hex string.
- `fn as_str(self: &Self) -> &str` - Returns the hex-encoded hash string.
- `fn into_string(self: Self) -> String` - Returns the hex-encoded hash string, consuming self.

**Traits:** Eq

**Trait Implementations:**

- **FromStr**
  - `fn from_str(s: &str) -> Result<Self, <Self as >::Err>`
- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **Ord**
  - `fn cmp(self: &Self, other: &DefinitionHash) -> $crate::cmp::Ordering`
- **PartialOrd**
  - `fn partial_cmp(self: &Self, other: &DefinitionHash) -> $crate::option::Option<$crate::cmp::Ordering>`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &DefinitionHash) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> DefinitionHash`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



