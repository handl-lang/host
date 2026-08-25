**handl_module_resolver_git**

# Module: handl_module_resolver_git

## Contents

**Modules**

- [`fetch`](#fetch)
- [`git_resolver`](#git_resolver)
- [`parse`](#parse) - Parse `git+` URLs into components.

---

## Module: fetch



## Module: git_resolver



## Module: parse

Parse `git+` URLs into components.

Format: `git+<scheme>://<host>/<org>/<repo>.git@<ref>/<path/to/file>`

Example:
```text
git+https://github.com/org/repo.git@main/schemas/base.yml
```
→ repo_url: `https://github.com/org/repo.git`
  ref: `main`
  path: `schemas/base.yml`



