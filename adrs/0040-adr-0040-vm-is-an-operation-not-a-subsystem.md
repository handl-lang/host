+++
id = "ADR-0040"
title = "ADR-0040: VM is an Operation, Not a Subsystem"
status = "open"
priority = "medium"
+++

---
title: VM is an Operation, Not a Subsystem
type: adr
date: 2026-09-02
status: accepted
supersedes: [ADR-0027]
status_note: Accepted — the VM is an operation; the VM/runtime crate families dissolve.
---

# ADR-0040: VM is an Operation, Not a Subsystem

## Decision

1. **Dissolve `plugin-vm-*` and `plugin-runtime-*`.** Evaluation uses the
   platform's `Eval` operation, `Behavior<Eval>` and `BasicEvaluator` — the same
   pattern as every other operation.
2. **No `VirtualMachine` trait, no `Value` trait, no `Runtime` operation, no `V`
   genericity**, and no `Expr`/`Pattern`/`HandlerClause` intermediate
   representation.
3. **Eval handlers are granular `Behavior<Eval>` plugins**, one per AST kind,
   each decoding `AstNode.payload` to `serde_json::Value`.
4. **`EvalRequest` carries `EvalContext`** (scopes, evaluator handle, modules,
   limits, call count); recursion is `ctx.eval(&child)` back through the
   evaluator.
5. **`EvalContext` lives in `plugin-ext-eval`**, `EvalRequest` in `platform`;
   eval is special at the request level, never at the trait level.
6. **Function registration collapses to `Module`/`ModuleRuntime`** with a
   non-generic `Function` trait over `serde_json::Value`.
7. **There is no bundled interpreter target**: the 11 `plugin-eval-*` plugins are
   the tree-walk target; a compiler target is a set of `Compile` rules instead.
8. **One cutover.** The old VM/runtime crates and `lego.rs`
   (`ExprHandler`/`LegoVm`) are deleted; no dual path.

## Why

The VM was a privileged subsystem with its own traits, genericity and IR, so it
re-implemented what `Behavior` already provides and blocked the JSON-boundary
model. Moving eval onto the operation pattern makes one dispatch path for every
stage and deletes a crate family.

## Not doing

- **Keep `VirtualMachine`/`Value` traits** — they existed only for the subsystem.
- **Keep `plugin-runtime-*`** — six crates duplicating `plugin-lib-*`.
- **Ship a bundled interpreter target** — the target is the set of eval plugins
  the kit loads.

## Consequences

- Every operation uses one behavior trait; eval is not special.
- Values are `serde_json::Value`; child nodes cross the JSON boundary.
- Targets are plugin sets: interpreter = `Eval` rules, compiler = `Compile`
  rules.
- The crate list shrinks; function registration has one mechanism.

## Enforcement

- `platform` — `EvalRequest`.
- `plugin-ext-eval` — `EvalContext`, the contract.
- `plugin-eval-*` — the 11 tree-walk rules; `plugin-module` — registration.

## Design

Handler mapping, crate taxonomy, context and the explicit not-kept list:
[`design/0040-vm-is-an-operation-not-a-subsystem.md`](../design/0040-vm-is-an-operation-not-a-subsystem.md).
