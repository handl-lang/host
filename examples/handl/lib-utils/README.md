# handl `lib-utils` curated examples

The full per-module `lib-utils` examples now live colocated with each crate,
under `crates/lib-utils/examples/<module>/`.

This directory keeps a few high-level/intro examples that do not map to a single crate.

- `hello.hnl` — uses `hello.greet()`, available via the `hello` kit in `.handl.toml`
- `greet.hnl` — same, with a different argument

## How `.handl.toml` works

The `.handl.toml` file tells `hnl` which kit(s) to load, which plugins to
exclude, and what option overrides to apply. It is read automatically when
you run commands that go through the plugin system (`eval`, `fmt`, `lint`,
`typecheck`, etc.).

**Note:** `hnl run` bypasses the plugin system and uses the VM directly,
so `.handl.toml` does not affect it. Use `hnl eval` instead to see the
config in action.

### This directory's config (`.handl.toml`)

```toml
[[kit]]
include = "hello"

[options]
"target" = "wasm"
```

- Loads the `hello` kit (canonical kit + `hello.greet()` support)
- Overrides the `target` option from `"native"` to `"wasm"`

### Demo 1: Kit loading

From this directory, `.handl.toml` is picked up automatically:

```sh
# Uses .handl.toml → loads hello kit → hello.greet is available
hnl eval 'hello.greet("world")'
# Output: Hello, world!
```

### Demo 2: Plugin exclusion

The file `.handl-no-fmt.toml` shows how `exclude` removes a plugin:

```toml
[[kit]]
include = "canonical"
exclude = ["fmt.basic"]
```

```sh
# With the default .handl.toml (fmt.basic is present):
hnl fmt 'hello.greet("world")'
# Works (no output = no formatter rules, but capability exists)

# With fmt.basic excluded:
hnl --config .handl-no-fmt.toml fmt 'hello.greet("world")'
# Error: UnknownCapability(CapabilityId("fmt"))
```

### Available kits

| Kit         | Description                                           |
| ----------- | ----------------------------------------------------- |
| `canonical` | Core language: lexer, parser, formatter, linter, etc. |
| `hello`     | Canonical + `hello.greet()` library                   |
| `cli`       | Canonical + OS utils (`fs.*`, `env.*`, `str.*`, etc.) |
| `config`    | Pure library plugins (no side effects)                |
| `server`    | Server-related plugins                                |

### `.handl.toml` fields

| Field        | Type         | Description                                                           |
| ------------ | ------------ | --------------------------------------------------------------------- |
| `[[kit]]`    | table array  | Kit to load. `include` = kit name, `exclude` = plugin ids to exclude  |
| `[[plugin]]` | table array  | Plugin to register. `register` = plugin id, `manifest` = path to JSON |
| `exclude`    | string array | Global plugin ids to exclude from composition                         |
| `[options]`  | key-value    | Option overrides keyed by option id                                   |
