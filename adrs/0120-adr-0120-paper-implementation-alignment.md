+++
id = "ADR-0120"
title = "ADR-0120: Paper/Implementation Alignment"
status = "open"
priority = "medium"
+++

---
title: Paper/Implementation Alignment
type: adr
date: 2026-09-18
status: accepted
status_note: Accepted — five ordered phases align the implementation with the paper.
---

# ADR-0120: Paper/Implementation Alignment

## Decision

1. **Phase 1 — arrows as values**: a `function` used as a value lowers to
   `Arr { value: Term }` (quoted, no environment capture); `call(f, x)` lowers
   to `seq(pair(quote f, x), app)`, while flow `x >>> f` stays `seq(x, f)`.
2. **Phase 1 — no self-application**: `desugar.data`'s recursor becomes
   `loop(step)` feedback, and there is no currying — parameters are products
   (partial application, if ever needed, is a prefix-splice of products).
3. **Phase 2 — composition axes**: `Term::Seq` gains an `axis` slot (default
   0); `>>>[k]` is grammar/YAML only, and types track globular source/target
   with `tgt_k(a) = src_k(b)`.
4. **Phase 3 — `dim` unification**: `ArrSource::Foreign(grade)` merges into the
   morphism dimension; `~[n]~>` means dimension.
5. **Phase 3 — effects as cells**: `op` is a cell and `handle` is a cell
   interpreter (an `(N+1)`-cell mapping an effectful `N`-cell to a pure one).
6. **Phase 4 — dependent structure**: `Family<A>`, `Π`, `Σ` enter via
   `types.yaml` + desugar; reindexing `f*` is composition, and the adjunctions
   are laws, not engine primitives.
7. **Phase 5 — laws as `M[2]` terms**: `f ~[2]~> g` parses to a `Term` and
   feeds `law_inference`; `Path`/`Equiv`/`Refine`/`witness`/`cell_*` are
   derived arrows, not primitives.
8. **Ordering is load-bearing**: `1 → 2 → 3 → (4, 5 in parallel)`, with the
   decision gates of the design doc at each step.

## Why

The implementation diverged from the paper in five places — captured
environments, self-application, axis-free composition, grade/dimension
confusion, and laws as a separate fragment — so the same program had two
readings. The phases restore one reading in dependency order.

## Not doing
- **Keep quoted functions capturing environments** — they are resolved at lower
  time.
- **Keep self-application as the recursor** — `loop` feedback replaces it.
- **Keep laws in a separate fragment** — they are `M[2]` terms.
- **Re-order the phases** — later phases depend on earlier ones.

## Consequences
- The paper and implementation agree at every phase boundary.
- Each phase has a gate; 4 and 5 can run in parallel.
- Derived arrows replace special-case ops.

## Enforcement
- `kernel/` — `Term` (`axis`, `Arr` quote), `desugar`.
- `crates/handl-kit-data-canonical/data/` — `types.yaml`, grammar.
- Phase gates in the design doc and parity tests.

## Design
+The five phases, their numbered changes and the decision gates:
+[`design/0120-paper-implementation-alignment.md`](../design/0120-paper-implementation-alignment.md).
