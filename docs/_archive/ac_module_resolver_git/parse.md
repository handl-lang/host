**handl_module_resolver_git > parse**

# Module: parse

## Contents

**Structs**

- [`GitTarget`](#gittarget) - Parsed components of a `git+` URL.

**Functions**

- [`parse_git_url`](#parse_git_url) - Parse a `git+` URL into [`GitTarget`].

---

## handl_module_resolver_git::parse::GitTarget

*Struct*

Parsed components of a `git+` URL.

**Fields:**
- `repo_url: String` - Repository URL without the `git+` prefix and without the `@ref/path` suffix.
- `reference: String` - Git reference (branch, tag, or commit hash).
- `path: String` - Path to the file within the repository.



## handl_module_resolver_git::parse::parse_git_url

*Function*

Parse a `git+` URL into [`GitTarget`].

Returns `None` if the URL is not a valid `git+` URL.

```rust
fn parse_git_url(input: &str) -> Option<GitTarget>
```



