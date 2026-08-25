**handl_lib_archive > entry**

# Module: entry

## Contents

**Structs**

- [`ArchiveEntry`](#archiveentry)

---

## handl_lib_archive::entry::ArchiveEntry

*Struct*

**Fields:**
- `path: String`
- `contents: Vec<u8>`

**Methods:**

- `fn new<impl Into<String>, impl Into<Vec<u8>>>(path: impl Trait, contents: impl Trait) -> Self`

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &ArchiveEntry) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> ArchiveEntry`



