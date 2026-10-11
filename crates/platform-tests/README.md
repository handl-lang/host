# handl-platform-tests

Integration tests for the `handl-platform` host. `crates/platform` ships unit
tests only; this crate is where cross-module behaviour is exercised —
`PluginRegistry` + `KitRegistry` + `DefaultHost` + the manifest-level
composition resolver.

```sh
cargo nextest run -p handl-platform-tests
```

## Layout

| File | Covers |
| --- | --- |
| `src/lib.rs` | `MockPlugin` (a `Plugin` that **records** the capabilities it served), `mock_manifest`, `request`. |
| `tests/mock.rs` | `Plugin` trait surface (`name`/`id`/`capabilities`/`metadata`) and registry dispatch reaching the plugin. |
| `tests/integration.rs` | Several plugins on one host, `load()` to a fixed point, per-capability dispatch, kit resolution by name and by composition, unknown-capability error. |
| `tests/e2e.rs` | A simulated host loads a kit and dispatches every capability the kit declares. |

## Restoration notes

The crate was deleted with the pre-composable platform (`bcf31afa "Remove
obsolete crates and files now superseded by the composable platform."`) and
restored here against the current API. Three things deliberately did **not**
come back:

- **`MockHost`** — `DefaultHost` already *is* the mutable plugin + kit host, so
  the tests drive the shipped host instead of shadowing it. This is strictly
  better: the tests now assert against real host behaviour.
- **`MockExtension` / `ExtensionRegistry`** — the `Extension` trait and its
  registry no longer exist anywhere in the workspace. `PluginContext` now
  carries only `plugins` and `kits`. (Extension-management is a *capability*
  now — see `plugin-ext-kit` / `plugin-ext-plugin`.)
- **`tests/language_host.rs`** — it targeted the removed `LanguageHost` /
  `SyntaxNode` and the deleted `handl-plugin-comments` /
  `handl-plugin-doc-comments`. Not restorable without a rewrite against the
  current LSP path (`plugin-lsp`, `plugin-ext-lsp`, `host-lsp`).

The sibling `crates/integration-tests` (fuzz/generics/integration) was **not**
restored: every import in it targets a deleted crate (`handl-ast-canonical`,
`handl-store`, `handl-vm*`, `handl-typecheck-core`/`-infer`/`-schema-json`,
`handl-lib-collections`). Its coverage lives in the per-crate conformance and
parity suites (`crates/*/tests/`) today.
