**handl_module_resolver_git > git_resolver**

# Module: git_resolver

## Contents

**Structs**

- [`GitResolver`](#gitresolver) - Git-based module resolver.

---

## handl_module_resolver_git::git_resolver::GitResolver

*Struct*

Git-based module resolver.

Clones repositories to a cache and reads files from the Git object
store (not the working tree). Two levels of caching:

1. **Repo cache** (internal) — keyed by `(repo_url, ref)`. Clones
   once per repo+ref; subsequent files from the same repo read from
   the cached clone after a lightweight fetch to refresh the ref.
2. **Module cache** (via [`ModuleCache`]) — keyed by
   `repo_url@ref/path`. Memoizes the extracted `ResolvedModule` so
   the same file is parsed only once.

Clones are bare and shallow (depth configurable via [`depth`]).
Cache paths use SHA-256 for cross-version stability.

**Generic Parameters:**
- C

**Fields:**
- `cache_dir: std::path::PathBuf`
- `module_cache: C`
- `repo_cache: std::sync::Mutex<std::collections::HashMap<String, std::path::PathBuf>>`
- `fetcher: crate::fetch::GitFetcher`

**Methods:**

- `fn with_cache(cache_dir: PathBuf, module_cache: C) -> Self` - Create a resolver with a specific cache directory and module cache.
- `fn temp_with_cache(module_cache: C) -> Self` - Create a resolver using the system temp directory and a module cache.
- `fn depth(self: Self, depth: usize) -> Self` - Set the shallow clone depth.
- `fn new(cache_dir: PathBuf) -> Self` - Create a resolver with a specific cache directory, no module cache,
- `fn temp() -> Self` - Create a resolver using the system temp directory and no module cache.

**Trait Implementations:**

- **ModuleResolver**
  - `fn resolve(self: &Self, path: &str, base: Option<&Path>) -> Option<ResolvedModule>`



