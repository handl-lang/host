**handl_lib_archive_tar > tar_archive**

# Module: tar_archive

## Contents

**Structs**

- [`TarArchive`](#tararchive) - Tar archive packer/unpacker.

---

## handl_lib_archive_tar::tar_archive::TarArchive

*Struct*

Tar archive packer/unpacker.

**Unit Struct**

**Methods:**

- `fn new() -> Self`

**Traits:** Copy

**Trait Implementations:**

- **Archive**
  - `fn pack(self: &Self, entries: &[ArchiveEntry]) -> Result<Vec<u8>, ArchiveError>`
  - `fn unpack(self: &Self, input: &[u8]) -> Result<Vec<ArchiveEntry>, ArchiveError>`
- **Clone**
  - `fn clone(self: &Self) -> TarArchive`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> TarArchive`



