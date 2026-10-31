+++
id = "ADR-0159"
title = "ADR-0159: Optimizer domains dissolve into `ext` tools, on a trigger"
status = "open"
priority = "medium"
+++

---
title: Optimizer domains dissolve into `ext` tools, on a trigger
type: adr
date: 2026-10-27
status: proposed
status_note: |-
  Proposed — 2026-10-27. Owners: `opt-graph` (was `graphopt`),
  `opt-kernel` (was `kernelopt`).
---

# ADR-0159: Optimizer domains dissolve into `ext` tools, on a trigger

## Context

The platform's shape is: an **engine** implements an operation, an **ext**
ships it as a command (`libhnl_ext_*.so` + manifest), and `hnl` is a dumb
host that discovers and dispatches. That is why `hnl` contains no tool
code.

The two optimizer domains do not follow it. `graphopt` and `kernelopt` are
Python + torch projects with their own drivers, CLIs and bench harnesses,
invoked directly by their author. Nothing outside them can call
"optimize this model" without knowing their internals.

Two facts pulled in opposite directions:

- **The endpoint is obvious.** An optimizer is a command. The product
  surface is `hnl optimize --domain graph` / `--domain kernel`, with the
  domain logic behind an ext and the research core behind a port.
- **The code is not ready.** It changes hourly; the referee is being
  built; and it is Python against a Rust host, so dissolving now means
  paying an FFI seam on a surface that has not stopped moving.

## Decision

**Adopt the endpoint, and gate it behind explicit triggers rather than a
date.**

1. **Domains stay labs for now.** `opt-graph` and `opt-kernel` keep their
   own CLIs, drivers and harnesses. No ext wrapper yet.
2. **Dissolve when all three triggers hold:**
   - a **caller that is not the researcher** (CI job, product, another
     domain) needs the capability;
   - the domain's public surface has been **unchanged for four weeks**;
   - the **referee is shared** (`bench/benchkit` usable by any domain),
     so a tool and a lab measure the same way.
3. **When triggered, wrap — do not rewrite.** The Python core is exposed
   through a port/adapter (`lib-triton`, `lib-*` family), and the ext is a
   thin command that calls it. Rewriting the search in Rust is not part of
   this decision.
4. **Naming:** an `opt-*` family (`opt-graph`, `opt-kernel`). A third
   member (e.g. `opt-schedule`) is created only on the rule of three — if
   the schedule space stays a module inside `opt-kernel`, it is not a
   domain.
5. **The generic core stays a library.** `egraph`, `discovery`, `eqsat`
   are domain-free; domains are rules + cost model + adapters. A new
   domain reuses the core rather than forking it.

## Visibility (ADR-0150)

ADR-0150 puts the tooling tier — `ext-*`, `host-cli`, `host-lsp`, `kit-*`,
the remaining `lib-*` ports — **private until done**. Dissolving a domain
into an `ext` therefore moves it into that tier. The consequence, stated so
it is not discovered later:

| artefact | tier | visible |
|---|---|---|
| `lib-kernel-ir` (ops, grades, costs, denotations) | vocabulary | **public** |
| `compile-triton` (Term → Triton) | tooling (like `compile-js`, `compile-llvm`) | **private** until done |
| `lib-triton` (runtime binding) | tooling | **private** |
| `ext-opt-*` (the command) | tooling | **private** |
| `opt-graph`, `opt-kernel` | labs | **private** (today: `graphopt` public, `kernelopt` private) |

So the public artefact of this whole line is the **vocabulary and its
denotations** — the thing that lets anyone state "this kernel means that
computation" — while the delivery stays private until it is done. That is
consistent with the platform's own split: public substrate, private
tooling.



- **Positive:** productization becomes packaging, not research; the
  freeze protects the research until the surface is stable; a second
  consumer forces the API to be honest.
- **Negative:** the FFI seam is real and will cost work at the trigger;
  until then, two invocation styles coexist (lab drivers vs `hnl`).
- **Neutral:** this ADR does not change the M1 plan (ADR-0160, the kernel
  IR, `compile-triton`, the equality gate, the search driver). It fixes
  *when and how* the domains become tools, not what they compute.
