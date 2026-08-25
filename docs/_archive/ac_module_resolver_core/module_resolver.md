**handl_module_resolver_core > module_resolver**

# Module: module_resolver

## Contents

**Traits**

- [`ModuleResolver`](#moduleresolver) - A module resolver — abstracts how import paths are loaded.

---

## handl_module_resolver_core::module_resolver::ModuleResolver

*Trait*

A module resolver — abstracts how import paths are loaded.

Implementations:
- `handl-module-resolver-file` — filesystem-based (`std::fs`)
- `handl-module-resolver-virtual` — in-memory (`HashMap`)
- `handl-module-resolver-http` — HTTP(S) via `ureq`
- `handl-module-resolver-git` — Git repositories via `git2`

**Methods:**

- `resolve`: Resolve a module path to its extracted fields + data.



