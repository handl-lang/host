**handl_module_resolver_core**

# Module: handl_module_resolver_core

## Contents

**Modules**

- [`module_cache`](#module_cache)
- [`module_caching_resolver`](#module_caching_resolver)
- [`module_chain_resolver`](#module_chain_resolver)
- [`module_resolver`](#module_resolver)
- [`resolved_module`](#resolved_module)
- [`resolved_module_test_helpers`](#resolved_module_test_helpers)

**Constants**

- [`IMPORT_FUNCTION`](#import_function) - The conventional name of the host function that triggers module resolution.

---

## handl_module_resolver_core::IMPORT_FUNCTION

*Constant*: `&str`

The conventional name of the host function that triggers module resolution.

This is a runtime host convention, not a language-level requirement.
The language defines no built-in functions — `import` is simply the name
the reference runtime uses for the function that loads external documents.
A runtime host may use a different name or provide module loading through
a different mechanism entirely.



## Module: module_cache



## Module: module_caching_resolver



## Module: module_chain_resolver



## Module: module_resolver



## Module: resolved_module



## Module: resolved_module_test_helpers



