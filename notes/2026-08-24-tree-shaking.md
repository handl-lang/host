# Tree-shaking in handl

## Two levels of tree-shaking

### 1. Crate-level (Cargo features) — DONE

The `handl` facade crate is now tree-shakeable via Cargo features. All 25+ sub-crates are `optional = true` with feature flags controlling inclusion. Consumers can do:

```toml
handl = { version = "0.1", default-features = false, features = ["yaml", "vm"] }
```

Core crates (`handl-ast`, `handl-dialect`, `handl-runtime`, `handl-source`) are always included. The `default` feature enables everything for backwards compatibility.

Feature groups: `vm`, `host`, `json`, `yaml`, `toml`, `fields`, `dialect-canonical`, `compiler`, `compiler-core`, `typecheck`, `typegen`, `resolver-core`, `resolver-memory`, `resolver-noop`, `resolver-file`, `resolver-virtual`, `lib-utils`, `lib-kv`, `lib-kv-hashmap`, `lib-platform`, `lib-platform-std`, `store`, `codebase`.

### 2. Language-level (DCE pass) — NOT IMPLEMENTED

The handl language itself has no tree-shaking / dead code elimination. Every declared field is processed:

- All `expr:` fields are computed (multi-pass dependency resolution)
- All `${...}` interpolations are resolved recursively
- All `merge:` fields are spread and removed
- All `import()` calls are loaded and cached

There is no reachability analysis — no concept of "reachable" vs "unreachable" fields. Every field in `$:` is part of the schema; every field in data is either declared or flagged as undeclared.

#### What exists today (not tree-shaking)

- **Typegen strips handl-specific keys** (`expr`, `arg`, `merge`, `inheritable`, `mutable`) from output schemas — schema cleanup, not DCE
- **Merge fields are removed** after spreading — semantic behavior, not optimization
- **Import caching** — same file loaded once even if `import()` called multiple times

#### What a DCE pass would look like

A reachability pass would:

1. Start from "output fields" — fields without `expr:` that appear in the final document, plus fields explicitly requested by the caller
2. Trace backward through `expr:` dependencies, `${...}` interpolations, and `import()` calls
3. Drop everything unreachable (unused fields, unused imports, unused function values)

The `Pass` trait in `handl-compiler-core` is designed for exactly this kind of addition. A custom pass slots into the pipeline without core changes:

```rust
let pipeline = Pipeline::new(config)
    .with_pass(ParsePass)
    .with_pass(ExtractPass)
    .with_pass(DeadFieldElimPass)  // custom — no core changes
    .with_pass(GeneratePass);
```

The pipeline docs even show this example with `DeadFieldElimPass` and `ConstantFoldPass` as placeholders (see `compiler-core/src/pipeline.rs` doc comments).

#### Challenges

- **Import side effects** — `import()` may load documents with function values that become available via path resolution. Determining if an import is "unused" requires tracing all path lookups, including those inside function bodies (which are strings evaluated lazily).
- **Function values are data** — functions are JSON objects with a `body` string. A function stored in a field might be called dynamically via `apply()`. Static reachability would need to trace through `apply` calls.
- **Host-provided functions** — the host decides which functions exist. A DCE pass can't know what side effects a host function has, so it must conservatively assume any host function call is reachable.
- **Interpolation is recursive** — `${...}` in strings inside arrays/objects. Reachability must trace through all nested structures.

#### Priority

Low — the language is designed for small configuration documents, not large programs. The overhead of unused fields is typically negligible. Tree-shaking becomes relevant if handl is used for large document graphs with many imports, where loading unused imports has real I/O cost.
