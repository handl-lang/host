---
title: Capability Discipline at the Language Level (ADR-0041 Phase 5) — design detail
type: design
date: 2026-08-30
updated: 2026-10-29
refs: [ADR-0046]
---

# Capability Discipline at the Language Level (ADR-0041 Phase 5) — design detail

Moved verbatim from ADR-0046 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

## Decision

### 1. `CapabilitySet` — a static capability summary of a `Language`

Add a type that summarizes the capabilities a `Language` requires:

```rust
/// The set of capabilities a Language value exercises.
/// Computed by scanning the Language's eval schemes for Foreign
/// dispatches and vau behaviors. Used for static sandboxing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    /// Host function names referenced by Foreign schemes.
    pub host_fns: BTreeSet<String>,
    /// Whether the language contains any vau (fexpr) behaviors.
    /// vau can call eval on arbitrary ASTs, so it implies eval capability.
    pub has_vau: bool,
    /// Whether the language can call eval (transitively, via meta fns).
    pub can_eval: bool,
    /// Whether the language can call compile.
    pub can_compile: bool,
}
```

Compute `CapabilitySet` by walking the `Language`'s eval schemes:

- `Scheme::Foreign { name, .. }` → add `name` to `host_fns`.
- `Scheme::Vau { .. }` → set `has_vau = true`, `can_eval = true`.
- If `host_fns` contains `"eval"` or `"run"` → `can_eval = true`.
- If `host_fns` contains `"compile"` → `can_compile = true`.

### 2. `CapabilityPolicy` — what a context allows

```rust
/// A policy declaring which capabilities are permitted.
/// Used by the host to sandbox eval/compile calls.
#[derive(Debug, Clone, Default)]
pub struct CapabilityPolicy {
    /// Allowed host function names. If empty, no host fns are allowed.
    pub allowed_host_fns: BTreeSet<String>,
    /// Whether vau behaviors are allowed.
    pub allow_vau: bool,
    /// Whether eval is allowed (recursive meta-evaluation).
    pub allow_eval: bool,
    /// Whether compile is allowed.
    pub allow_compile: bool,
}
```

### 3. Static check at `eval` / `run` / `compile` boundary

Before `eval(handle, ast)` dispatches, the host function checks:

```rust
fn check_capabilities(lang: &Language, policy: &CapabilityPolicy) -> Result<(), LangError> {
    let caps = CapabilitySet::from_language(lang);
    if caps.has_vau && !policy.allow_vau {
        return Err(LangError::CapabilityDenied { cap: "vau".into() });
    }
    if caps.can_eval && !policy.allow_eval {
        return Err(LangError::CapabilityDenied { cap: "eval".into() });
    }
    if caps.can_compile && !policy.allow_compile {
        return Err(LangError::CapabilityDenied { cap: "compile".into() });
    }
    for fn_name in &caps.host_fns {
        if !policy.allowed_host_fns.contains(fn_name) {
            return Err(LangError::CapabilityDenied { cap: fn_name.clone() });
        }
    }
    Ok(())
}
```

This is a **static** check — it inspects the `Language` data, not the
running program. No runtime sandbox, no instrumentation, no overhead during
execution. The check happens once at the `eval` boundary.

### 4. `LanguageRegistry` carries policies

The `LanguageRegistry` maps handles to `(Language, RuntimeConfig,
CapabilityPolicy)`. When `eval(handle, ast)` is called:

1. Look up `(lang, config, policy)` for `handle`.
2. Check `lang`'s capabilities against `policy`.
3. If denied, return `LangError::CapabilityDenied`.
4. If allowed, build the sub-runtime and evaluate.

The policy is attached at **registration time**, not at eval time. The host
that builds the registry decides what each language is allowed to do.

### 5. Capability inheritance for sub-runtimes

When `eval` creates a sub-runtime, the sub-runtime's `LanguageRegistry` is
built with a **narrowed** policy: the sub-language cannot exceed the
parent's capabilities. If the parent language has `allow_eval = true` but
`allow_compile = false`, the sub-language's policy also has
`allow_compile = false`.

This prevents capability escalation: a sandboxed language cannot `eval`
a more-privileged language, because the sub-runtime's registry is built
with the parent's policy constraints.

### 6. `capabilities(lang)` as a handl-callable function

Expose capability inspection to handl programs:

```handl
mod-lang = load("combat-mod.hnl")
caps = capabilities(mod-lang)
safe = caps.subset-of(allowed-mod-caps)
if not safe then reject else load(mod-lang)
```

This is a read-only host function: `capabilities(handle) → CapabilitySet`
(serialized as JSON). It lets a handl program make access-control decisions
about other `Language` values before evaluating them.

### 7. Implementation phases

1. **`CapabilitySet::from_language`** — scan eval schemes, compute the set.
   No enforcement yet; just inspection.
2. **`CapabilityPolicy` on `LanguageRegistry`** — attach policies at
   registration, check at eval boundary.
3. **Capability inheritance** — narrow sub-runtime policies.
4. **`capabilities(handle)` host function** — expose to handl programs.
5. **`CapabilityDenied` error type** — surface to the program.

## Why

ADR-0041 made `Language` a first-class value and `eval`/`self`/`compile` handl-callable functions. ADR-0044 defined `Language` as pure serializable data — grammar, eval schemes, token kinds, all as data fragments. The canonical `Language` is assembled from YAML files.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Static sandboxing.** No runtime overhead — the check happens once at
- **Composable.** Policies narrow on sub-runtime creation, preventing
- **Inspectable.** `capabilities(lang)` lets handl programs make
- **Data-driven.** The capability set is computed from `Language` data
- **Over-approximation.** `CapabilitySet` lists all capabilities the

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0046-capability-discipline-at-the-language-level.md`](../design/0046-capability-discipline-at-the-language-level.md).
