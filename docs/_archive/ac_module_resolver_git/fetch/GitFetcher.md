**handl_module_resolver_git > fetch > GitFetcher**

# Module: fetch::GitFetcher

## Contents

**Functions**

- [`cache_path`](#cache_path) - Compute the local cache path for a repo URL + ref using SHA-256.
- [`fetch_file`](#fetch_file) - Clone or open a repo, then read a file from it.

---

## handl_module_resolver_git::fetch::GitFetcher::cache_path

*Function*

Compute the local cache path for a repo URL + ref using SHA-256.

SHA-256 is stable across Rust versions, unlike `DefaultHasher`.

```rust
fn cache_path(self: &Self, target: &GitTarget) -> PathBuf
```



## handl_module_resolver_git::fetch::GitFetcher::fetch_file

*Function*

Clone or open a repo, then read a file from it.

If the repo is already cached, performs a fetch to refresh the ref
before reading. New clones are bare and shallow.

```rust
fn fetch_file(self: &Self, target: &GitTarget) -> Option<String>
```



