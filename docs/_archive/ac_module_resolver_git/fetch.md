**handl_module_resolver_git > fetch**

# Module: fetch

## Contents

**Structs**

- [`GitFetcher`](#gitfetcher) - Git fetch helper — clones, opens, and reads files from a Git repository.

**Constants**

- [`DEFAULT_DEPTH`](#default_depth) - Default shallow clone depth.

---

## handl_module_resolver_git::fetch::DEFAULT_DEPTH

*Constant*: `usize`

Default shallow clone depth.



## handl_module_resolver_git::fetch::GitFetcher

*Struct*

Git fetch helper — clones, opens, and reads files from a Git repository.

Encapsulates the low-level `git2` operations: cache path computation,
bare shallow cloning, ref refreshing, and blob reading.

**Fields:**
- `cache_dir: std::path::PathBuf`
- `depth: usize`

**Methods:**

- `fn new(cache_dir: PathBuf, depth: usize) -> Self`
- `fn cache_path(self: &Self, target: &GitTarget) -> PathBuf` - Compute the local cache path for a repo URL + ref using SHA-256.
- `fn fetch_options(self: &Self) -> git2::FetchOptions` - Build fetch options with the configured depth.
- `fn fetch_file(self: &Self, target: &GitTarget) -> Option<String>` - Clone or open a repo, then read a file from it.
- `fn fetch_file_at(self: &Self, target: &GitTarget, local: &Path) -> Option<String>` - Like [`fetch_file`](Self::fetch_file) but uses a caller-supplied cache



