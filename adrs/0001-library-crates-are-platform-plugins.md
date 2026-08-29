# Library crates are exposed as Platform plugins

## Context

The `crates/` directory contains two overlapping categories:

- `lib-*` crates: standard library implementations (e.g. `lib-json`, `lib-crypto`, `lib-hello`).
- `plugin-*` crates: plugins that contribute rules to the host (e.g. `plugin-eval-hello`, `plugin-runtime-arithmetic`, `plugin-grammar-array`).

Currently only `lib-hello` follows a registration pattern where the crate itself implements the `Plugin` trait. Its `HelloLibraryPlugin` contributes a concrete `HelloPlugin` to the host so that `hello.greet` is available at runtime. The majority of `lib-*` crates are plain libraries and are not directly loadable by the host, which makes the boundary between "library" and "plugin" ambiguous.

## Decision

Every standard-library crate is a Platform plugin:

1. The crate name is prefixed with `plugin-lib-` (e.g. `plugin-lib-json`, `plugin-lib-crypto`) to make the plugin role explicit and consistent with the `plugin-*` family.
2. Each crate contains a `plugin` module that exposes a minimal `Plugin` implementation.
3. The plugin's `id` is `lib.<name>` (e.g. `lib.json`), and its `contribute` method registers the concrete library implementation(s) with the host.
4. The plugin does not declare capabilities of its own unless the library is directly invokable; its purpose is to add the library's rules/modules to the host.

## Consequences

- `plugin-*` crates that consume a library (e.g. `plugin-eval-hello`) will depend on `plugin-lib-*` crates instead of `lib-*` crates.
- All library crates gain a runtime identity, which simplifies host loading, kit manifests, and dependency resolution.
- The refactor touches the workspace `Cargo.toml`, all `lib-*` crate directories, and every crate that depends on them.
- The `lib-` prefix is removed from crate names, so references in docs and kits must be updated.

## Status

accepted
