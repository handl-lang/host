+++
id = "ADR-0169"
title = "ADR Review and Archive Summary"
status = "open"
priority = "medium"
+++

---
title: ADR Review and Archive Summary
type: report
date: 2026-09-01
---

# ADR Review and Archive Summary

**Date:** 2026-09-01  
**Task:** Review, analyze, and archive ADRs based on implementation status

## Summary of Actions

### ✅ Archived (Fully Complete)

1. **ADR-0001 (Remaining) → Moved to archived/**
   - **Status:** Complete (superseded by ADR-0044 data-driven architecture)
   - **File:** `archived/0001-generic-node-and-plugin-driven-canonical-kit-remaining-archived.md`
   - **What was done:** Canonical kit cleaned up via YAML data files (tokens.yaml, grammar.yaml, etc.) in `kit-data-canonical` instead of plugin crates
   - **Why moved:** Original goal achieved, though via different approach

### 🔄 Split into "Done" + "Remaining"

#### 2. ADR-0002 (Plugin Model)

- **Archived:** `archived/0002-handl-platform-composable-host-v1-complete.md`
  - ✅ Unified `Plugin` model implemented
  - ✅ `Behavior<Op>` trait working (30+ implementations across codebase)
  - ✅ Legacy `Extension`/`CommandProvider`/`Library` removed
  - ✅ Host is pure composer with `PluginRegistry`
- **Remaining:** `0002-handl-platform-composable-host-remaining.md` (updated)
  - ❌ plugin-lib-* crate conversion (20+ crates using Module/Function pattern)
  - ❌ Decision needed: Should these use `Behavior<Op>` or stay as Module/Function abstraction?
  - ❌ Library dependency expression and options handling

#### 3. ADR-0025 (HTTP Server)

- **Archived:** `archived/0025-http-server-v1-complete.md` (new)
  - ✅ Client crates renamed (lib-http → lib-http-client)
  - ✅ Module async/operation dispatch extended
  - ✅ Abstract server interface (`HttpServer` trait)
  - ✅ Concrete std implementation with tokio/hyper
  - ✅ HTTP/2 support (via hyper features)
  - ✅ Trie-based routing with matchit
- **Remaining:** `0025-http-server-remaining.md` (updated)
  - ❌ v1.x: FunctionValue route handler dispatch (stub placeholder)
  - ❌ v2: TLS termination
  - ❌ v2: Middleware hooks
  - ❌ v2: Compression (gzip/brotli)
  - ❌ v2: CORS and rate-limiting

#### 4. ADR-0032 (Types as Values)

- **Archived:** `archived/0032-types-as-values-v1-complete.md` (new)
  - ✅ Shared Value representation (runtime + typechecker)
  - ✅ Base collections (type.number, type.string, type.bool, etc.)
  - ✅ Propositional equality (===)
  - ✅ Universe levels (Type_0, Type_1, etc.) with cumulativity
  - ✅ Dependent types (Pi, Sigma) fully working
  - ✅ NBE typechecker with proper level handling
- **Remaining:** `0032-types-as-values-remaining.md` (updated)
  - ❌ `data` declarations for inductive types
  - ❌ `rec` / termination checking
  - ❌ Inductive families (Vec(A, n))
  - ❌ Dependent pattern matching with motive
  - ❌ Type inference and unification (_ holes)
  - ❌ Proof erasure at runtime
  - ❌ Tactics (auto, induction, rewrite, simp)

### ❌ Not Done (Kept as Remaining)

5. **ADR-LSP (Remaining)**
   - **Status:** Not started
   - **File:** `ADR-LSP-remaining.md` (unchanged)
   - **Missing:**
     - ❌ `plugin-lsp-hover` crate (doesn't exist)
     - ❌ `plugin-lsp-source-position` crate (doesn't exist)
   - **Note:** LSP plugins exist for completion, goto, symbol but not hover/source-position

---

## Files Changed

### Archived (New)

- `project/adrs/archived/0001-generic-node-and-plugin-driven-canonical-kit-remaining-archived.md` (moved)
- `project/adrs/archived/0002-handl-platform-composable-host-v1-complete.md` (new)
- `project/adrs/archived/0025-http-server-v1-complete.md` (new)
- `project/adrs/archived/0032-types-as-values-v1-complete.md` (new)

### Updated (Refined)

- `project/adrs/0002-handl-platform-composable-host-remaining.md`
  - Clarified library plugin conversion as the remaining work
  - Added architectural questions to resolve
- `project/adrs/0025-http-server-remaining.md`
  - Removed HTTP/2 and matchit (already implemented)
  - Split v1.x completion from v2 enhancements
- `project/adrs/0032-types-as-values-remaining.md`
  - Noted that universe levels are now complete
  - Focused on advanced features (inductive types, tactics, etc.)

---

## Verification Checklist

- ✅ ADR-0001-remaining moved to archived/ with status explained
- ✅ ADR-0002 split: v1-complete created, remaining updated with open questions
- ✅ ADR-0025 split: v1-complete created, remaining updated to remove completed items
- ✅ ADR-0032 split: v1-complete created, remaining updated to focus on advanced features
- ✅ ADR-LSP-remaining kept as-is (not yet started)
- ✅ All archive files include "Related work" links back to remaining ADRs
- ✅ All remaining ADRs updated to link to their v1-complete archives

---

## Next Steps (For Contributors)

1. **ADR-0002:** Decide on library plugin abstraction (Behavior<Op> vs Module/Function)
2. **ADR-0025:** Complete v1.x FunctionValue dispatch, then implement v2 features
3. **ADR-0032:** Start with `data` declarations and termination checking
4. **ADR-LSP:** Implement hover and source-position plugins

---

## Related ADRs (For Context)

- ADR-0040 (VM is an operation) — explains ExecControl exception
- ADR-0044 (Language as data) — explains why ADR-0001 uses YAML instead of plugin crates
- ADR-0054 (Generic typecheck engine) — builds on ADR-0032
