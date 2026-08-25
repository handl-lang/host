# handl_module_resolver_noop

# handl-module-resolver-noop

No-op [`ModuleCache`] implementation — every `get` returns `None`.

Use when caching is not desired (e.g., tests that need fresh I/O
on every resolve, or memory-constrained environments).

## Modules

### [`handl_module_resolver_noop`](handl_module_resolver_noop.md)

*1 module*

### [`no_cache`](no_cache.md)

*1 struct*

