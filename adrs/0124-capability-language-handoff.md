---
title: Handing a `Language` to capability plugins
type: adr
date: 2026-10-10
status: proposed
status_note: |-
  **Proposed, revised.** Not implemented. The first revision was wrong about
  `Language` being non-serializable (see §Correction) and therefore proposed
  more machinery than the problem needs; the revision above is the corrected
  version and needs **no `platform` change**.

  Open question for review: whether per-call deserialization of ~180 KiB is
  acceptable for the LSP, or whether the side-channel should be built up front.
  Recommendation: ship the payload path, measure, then decide.
---


# ADR-0124: Handing a `Language` to capability plugins

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



## Why

The `plugin-ext-*` tooling plugins (`fmt`, `lint`, and the proposed `typecheck`/`test`) split into two groups:

## Not doing

- **`platform` depends on `handl-language`; `CapabilityRequest.language:
- **Payload carries the *fragments* (the kit recipe) and the plugin
- **The plugin builds the canonical language itself.** Works for `fmt`/`lint`
- **Keep language-bound operations out of the capability layer.** Leaves

## Consequences

- **No `platform` change.** The seam stays as it is; nothing new to learn.
- **Portable** — works in-process, in WASM, and in a child process. The
- A host serializes once and reuses, so the 180 KiB is a startup cost, not a
- Precedent exists: `plugin-engine-typecheck-handl` already marshals a
- ~180 KiB compact JSON (542 KiB pretty) per distinct language, plus a

## Enforcement

- `plugin-ext-*` — where this is true.
- `plugin-engine-typecheck-handl` — where this is true.
- `host-lsp` — where this is true.
- `kit-canonical` — where this is true.
- `host-cli` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0124-capability-language-handoff.md`](../design/0124-capability-language-handoff.md).
