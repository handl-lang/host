---
title: The kernel IR is a Term plus kit-declared ops
type: adr
date: 2026-10-29
status: implemented
applies_to: '`lib-kernel-ir` (ops, terms, denotations), `kernelopt` (search), `graphopt/bench` (gate)'
---

# ADR-0165: The kernel IR is a `Term` plus kit-declared ops

## Decision

> Numbered 0165, not the 0160 the M1 build list asked for: `0160` is
> already `lib-repos-are-language-packages`, and numbers are never reused
> (ADR-0161 rule 6).

1. A kernel body is a **`Term`**: an op name applied to arguments, where
   arguments are other terms or leaves from an environment. The IR is data.
2. The **op set is closed at nine** — `tile_load`, `unpack_i4`, `dequant`,
   `dot`, `split`, `stage`, `reduce`, `store`, `bias`. Adding a tenth is a
   finding about the platform, not a commit.
3. Every op declares four things: a **signature**, a **grade**
   (`EXACT` or `ROUNDS`), a **cost** (bytes, ops, launches), and a **torch
   denotation** — one reference function per op.
4. Two terms are equal iff their **denotations** are equal, checked by
   `bench.benchkit.equality.cosine_gate` against an **external**
   incumbent (`awq_gemm`). A term is never gated against another term we
   wrote.
5. The IR carries **no kernel combinators**. It enters the platform as a
   language and is interpreted by an engine (`compile-triton`); the
   kernel's six combinators stay closed.
6. Cost is a **hard bound only** — roofline floor and legality pruning.
   It never ranks feasible candidates; the referee does.
7. The denotation is the *meaning*, not the *schedule*. Two equal terms
   may differ 3× in time and that is the point of searching over them.

## Why

The AWQ nibble-order bug passed a full round of "exactness" checks because
kernel and reference shared an assumption (cosine 0.21 against `awq_gemm`,
P21/P22). A body that is data, with a declared per-op denotation and an
external gate, makes that class of failure a test rather than a memory. It
also puts the search over something a human can read: `split_k = 4` is
four `split` terms with a provably equal composed denotation, not a flag
someone hopes is safe.

## Not doing

- **A tenth op for pipelining or shared-memory residency** — the review's
  strongest objection (the nine ops cannot express scheduling); rejected
  for M1 because the codegen leg is unproven and a schedule language that
  cannot beat 30 lines of Triton is decoration. Revisit after M1.
- **Proving schedules equal** — denotations prove meaning only; cost is
  measured, never derived (ADR: residual opacity).
- **An open op registry** — `OPS` is a frozen mapping; extension is an ADR.
- **Ranking candidates by the cost model** — measured to contradict deploy
  ranking on three of four shapes (P10/P12).

## Enforcement

- `lib-kernel-ir/src/lib_kernel_ir/ops.py` — `OPS`, the frozen nine, each
  with signature/grade/cost/denotation.
- `lib-kernel-ir/tests/test_ops.py` — nine denotation tests + "all four
  `split_k` terms denote the same function" at cosine ≥ 0.99999.
- `graphopt/bench/benchkit/equality.py` — the external-incumbent gate.
- `kernelopt/src/mlopt_kernel/search.py` — enumerates terms, applies the
  gate, prunes on cost, measures with the referee.
