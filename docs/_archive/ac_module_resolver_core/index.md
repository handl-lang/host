# handl_module_resolver_core

# handl-module-resolver-core

Module resolution abstraction.

Defines the [`ModuleResolver`] trait and [`ResolvedModule`] struct.
Implementations live in separate crates:

- `handl-module-resolver-file` — filesystem-based (`std::fs`)
- `handl-module-resolver-virtual` — in-memory (`HashMap`)
- `handl-module-resolver-http` — HTTP(S) via `ureq`
- `handl-module-resolver-git` — Git repositories via `git2`
- `handl-module-resolver-memory` — in-memory `ModuleCache` implementation
- `handl-module-resolver-noop` — no-op `ModuleCache` implementation

Any system that needs to follow `import()` calls (typegen, validation,
linting) can take `&dyn ModuleResolver` without caring where modules
come from.

## Modules

### [`handl_module_resolver_core`](handl_module_resolver_core.md)

*1 constant, 6 modules*

### [`module_cache`](module_cache.md)

*1 trait*

### [`module_caching_resolver`](module_caching_resolver.md)

*1 struct*

### [`module_chain_resolver`](module_chain_resolver.md)

*1 struct*

### [`module_resolver`](module_resolver.md)

*1 trait*

### [`resolved_module`](resolved_module.md)

*1 struct*

### [`resolved_module_test_helpers`](resolved_module_test_helpers.md)

*1 function*

