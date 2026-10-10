# ADR-0124: Handing a `Language` to capability plugins

## Context

The `plugin-ext-*` tooling plugins (`fmt`, `lint`, and the proposed
`typecheck`/`test`) split into two groups:

- **language-free** — `format_source(src)`, `lint_source(src)`. They build the
  canonical language internally. These already work through the capability
  seam (`FmtPlugin`, `LintPlugin`).
- **language-bound** — `format_source_with_language(lang, src)`,
  `lint_source_with_prelude_and_language(lang, src)`,
  `check_module(lang, src)`, and the test runner. They take the `Language`
  from their **caller**.

The second group cannot be expressed today, so `hnl fmt --language <name>`,
`hnl typecheck`, and `hnl test` — the cases that matter most — have no path
through the contract, and `host-lsp` (which works with workspace language
variants, ADR-0003) cannot reuse the tooling at all. This is the blocker
recorded in
[`project/tasks/0085-tooling-capability-wiring.md`](../tasks/0085-tooling-capability-wiring.md).

### Correction: a `Language` *is* serializable

The first version of this ADR claimed a `Language` is "a large, non-`serde`
Rust value" and reasoned from there. **That was wrong**, and the correction
changes the decision. Verified:

- `Language`'s own doc reads *"A language definition as pure serializable
  data"*, and it derives `Serialize, Deserialize` with **no `serde(skip)`**
  fields (`handl-language/src/language.rs`).
- It is **already marshalled to JSON today**:
  `plugin-engine-typecheck-handl::marshal_language` does
  `serde_json::to_value(lang)` and hands the result to the handl-written
  typechecker model.
- Measured size of the canonical language: **180 KiB** compact JSON
  (542 KiB pretty), across 105 token rows, 85 grammar, 87 eval, 82 types,
  94 AST shapes, 102 lexer rules, 16 lints.

So the payload *can* carry a language. What it cannot carry is an `Arc` to an
already-built one — which is a cost question, not a representability one.

### The constraint that still holds

**`platform` deliberately does not depend on `handl-language`**, and its `Kit`
is manifest-level (`KitManifest` + `Vec<PluginManifest>`) with no language
payload. Any solution that puts a typed `Language` *field* on the request
therefore requires `platform` to link the language layer — the same reason a
plain `Arc<Language>` field was rejected.

## Decision

**Primary: the payload carries the serialized language.** No `platform`
change at all.

```json
{ "source": "…", "checks": true, "language": { …serialized Language… } }
```

- A host serializes its language **once** and reuses it (an `Arc<Value>` or
  `Arc<str>`), rather than re-encoding per call, so the 180 KiB is paid once,
  not per file.
- A plugin that needs a language reads `payload["language"]`, deserializes it,
  and proceeds. A plugin that cannot find one *and* cannot build a suitable
  default fails with an actionable error rather than silently using canonical.
- This path is **portable**: it works for in-process hosts, WASM plugins, and
  child-process plugins alike. The side-channel below is not.

**Optional optimisation, only if measured necessary: an in-process
side-channel.** For hot paths (the LSP re-linting on every keystroke),
deserializing 180 KiB per call may be too slow. If so, add a non-serialized
`TypeId`-keyed slot on `CapabilityRequest` holding an `Arc<Language>`, which
`platform` can express without depending on `handl-language` (it only sees
`Any`). Plugins consult it *first*, then fall back to the payload. This is a
cache, not the interface.

### Optional: the in-process side-channel (platform, language-agnostic)

Only if the payload path measures too slow. `platform` can host it without
depending on `handl-language`, because it only ever sees `Any`.

```rust
pub struct CapabilityRequest {
    pub capability: CapabilityId,
    pub payload: serde_json::Value,
    /// In-process host objects. Never serialized: these are `Arc`s to live
    /// runtime values (a `Language`, a resolver, …) that cannot cross a
    /// process boundary. Empty for out-of-process callers.
    #[serde(skip)]
    host_objects: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl CapabilityRequest {
    pub fn insert_host_object<T: Any + Send + Sync>(&mut self, v: Arc<T>);
    pub fn host_object<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
}
```

`platform` depends only on `Any`, so it stays language-agnostic.

### 2. A resolver trait in the language layer

```rust
/// Resolves a kit name (or the host default when `None`) to a language.
pub trait LanguageResolver: Send + Sync {
    fn language_for(&self, kit: Option<&str>) -> Result<Arc<Language>, String>;
}
```

The host inserts its implementation (the same data that already backs
`hnl --language <name>`), so a **custom kit** resolves, not just canonical.

### 3. A single accessor the plugins use

```rust
fn request_language(req: &CapabilityRequest, ctx: &PluginContext)
    -> Result<Arc<Language>, PlatformError>;
```

Resolution order:

1. an `Arc<Language>` placed directly on the request (fast path, exact);
2. the request's `LanguageResolver`, asked for `payload["kit"]` (or the
   default when absent/`null`);
3. otherwise `PlatformError::InvalidMetadata` naming *both* ways to supply a
   language, so a plugin author who forgets gets an actionable error rather
   than a silent canonical fallback.

A new optional payload field makes the request self-describing for
out-of-process callers:

```json
{ "source": "…", "kit": "config" }
```


## Consequences

**Positive (primary path: payload carries the serialized language).**

- **No `platform` change.** The seam stays as it is; nothing new to learn.
- **Portable** — works in-process, in WASM, and in a child process. The
  side-channel could not do this.
- A host serializes once and reuses, so the 180 KiB is a startup cost, not a
  per-call one.
- Precedent exists: `plugin-engine-typecheck-handl` already marshals a
  `Language` to JSON for the handl typechecker, so this is not a novel burden
  on the platform — it is the mechanism already in use for one consumer.

**Negative / constraints.**

- ~180 KiB compact JSON (542 KiB pretty) per distinct language, plus a
  deserialize per plugin call. Per-call deserialization is the real risk; it
  is why the side-channel is kept as an escape hatch rather than deleted.
- A language with heavy FFI/module dependencies still is not *self-sufficient*
  after deserialization: `fmt`/`lint`/`typecheck` need only the data, but
  `eval` additionally needs the plugin-provided module registry. The handoff
  solves the data problem, not the capability problem — say so plainly so
  nobody expects a deserialized language to evaluate.
- Malformed or version-skewed `language` payloads need a clear error
  (`scheme_version`, ADR-0044 §7, exists to detect the latter).

## Alternatives rejected

- **`platform` depends on `handl-language`; `CapabilityRequest.language:
  Option<Arc<Language>>`.** Simplest to write, but makes the generic plugin
  host depend on the language layer — every host, including ones that never
  touch handl syntax, would link it. Rejected on layering.
- **Payload carries the *fragments* (the kit recipe) and the plugin
  re-assembles.** Serializable and smaller than the assembled form, but it
  re-runs kit assembly per call, and the recipe is not always available to a
  plugin that was handed a built `Language` by its host.
- **The plugin builds the canonical language itself.** Works for `fmt`/`lint`
  defaults, fails the custom-kit and `host-lsp` cases that motivate the work,
  and would force a *linter* to depend on `kit-canonical`.
- **Keep language-bound operations out of the capability layer.** Leaves
  `host-lsp` unable to reuse the tooling, i.e. fails the stated goal.

## Migration

1. Define the `language` payload field; add a host helper that serializes the
   language once and caches it for the run.
2. Add `typecheck` and `test` plugins reading it; extend `fmt`/`lint` to accept
   the optional language (absent = today's canonical default, unchanged).
3. Reroute `host-cli`'s `--language` variants and `hnl typecheck`/`hnl test`
   through the seam.
4. **Test:** a *custom* kit driven through the capability path must produce
   identical results to the direct call. Without that, "the seam is
   equivalent" is untested — the same gap ADR-0123's acceptance test closed.
5. Benchmark the LSP path. Add the side-channel only if the deserialize cost
   shows up.

## Status

**Proposed, revised.** Not implemented. The first revision was wrong about
`Language` being non-serializable (see §Correction) and therefore proposed
more machinery than the problem needs; the revision above is the corrected
version and needs **no `platform` change**.

Open question for review: whether per-call deserialization of ~180 KiB is
acceptable for the LSP, or whether the side-channel should be built up front.
Recommendation: ship the payload path, measure, then decide.
