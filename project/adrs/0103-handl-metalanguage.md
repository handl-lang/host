# ADR-0103: Handl as Its Own Metalanguage

## Status

Proposed.

## Context

ADR-0101 made `Term` and `Language` first-order serializable data and ADR-0102 showed how to express new type-theoretic constructs (`Session` grades) as data over the existing arrow calculus. The `Model` trait is generic: it is the interface between the host runtime and a model of `Term` semantics. The current engines (`ValueModel`, `TypeModel`, `CompileModel`, optimization passes, etc.) are written in Rust.

The goal is to move the metalanguage inward. Handl itself should describe new languages and new engines. A new `Language` is already a data fragment; the next step is for an engine to be a Handl arrow that the host runtime loads, compiles, and executes through the generic `Model` boundary.

This ADR records the staged bootstrap that turns Handl into its own metalanguage.

## Decision

### 1. The `Model` boundary is the only fixed host interface

The host keeps one fixed seam: a `Model` is an object that, given a `Term`, an incoming `Value`, and a `ModelCtx`, produces a `Value` (or an effect). All higher-level engine logic is supplied from the outside, as a Handl module.

The host may ship built-in models for the earliest bootstrap stages, but they are not privileged at the interface level. Any Handl arrow that matches the model signature can be registered and run.

### 2. Engines are Handl arrows

An engine is a Handl module that exports a `run` arrow:

```handl
op run : (Term, Value, ModelCtx) ~> Value
```

Variants of the signature are allowed for different engine kinds (e.g. a type model may return `Type`, an optimizer may return `Term` or a `RewriteTrace`). The concrete input/output types are declared in the module and matched at registration time.

At runtime, an engine is registered under a `handl:engines.<name>` namespace. For example:

```handl
kit/runtime {
    eval = "handl:engines.value_model"
}
```

### 3. Runtime loading and wrapping

When the runtime is asked for `handl:engines.<name>`, it:

1. Locates and loads the Handl module `engines/<name>.hnl`.
2. Looks for the canonical `run` arrow in the module's exports.
3. Compiles the arrow through the current active compiler (initially a Rust compiler; later a Handl compiler from the previous stage).
4. Wraps the compiled artifact in a Rust `Model` implementation.
5. Caches the compiled engine artifact keyed by the `Term` hash of the module's `run` arrow.

The runtime treats the wrapper as the `Model` for that engine kind, dispatching `Term` evaluation to the compiled Handl arrow.

### 4. Bootstrap stages

The move from Rust engines to Handl engines proceeds in four stages. Each stage produces a new compiler or engine that the next stage uses.

1. **Rust `TreeWalkEngine` and `CraneliftEngine`**
   Two reference engines remain in Rust. `TreeWalkEngine` is the interpreter of last resort; `CraneliftEngine` is the first native-code compiler. They are the stage-0 seed.

2. **Handl `ValueModel` arrow, compiled by Rust `CraneliftEngine` to native**
   The value interpreter is reauthored as a Handl arrow `engines/value_model.hnl`. Stage 1 compiles it and the runtime uses the compiled artifact for normal evaluation.

3. **Handl `BytecodeCompiler` arrow, compiled by stage 2**
   The bytecode compiler is reauthored in Handl. Stage 2 (the Handl value model) runs it to produce bytecode, which the runtime can then install.

4. **Handl `CraneliftCompiler` arrow, compiled by stage 3 (or stage 0 the first time)**
   The Cranelift native compiler is reauthored in Handl. On first bootstrap it is compiled by the Rust `CraneliftEngine` (stage 0); after that, stage 3 compiles itself using the previous `CraneliftCompiler` artifact.

Stage 4 is self-hosting: the compiler is a Handl program compiled by the previous version of the compiler.

### 5. `Op` primitives form the fixed host ISA

The primitive `Op` vocabulary remains the host instruction set. Higher engine logic (term dispatch, environment handling, rewrite tracing, optimization heuristics) is written as Handl arrows composed from these primitives. New languages are expressed as data fragments over `Term`; new engines are Handl modules over `Op`.

### 6. Performance is solved by compilation (Futamura projection)

An engine written in Handl is an interpreter. The bootstrap stages apply the Futamura projection: a Handl compiler compiles the interpreter together with a program (here, the engine source `Term`) to produce a native engine. Because `Term` is serializable and the `Model` boundary is fixed, compiled engine artifacts can be cached and reused across runs.

### 7. Residual host surface

Not everything moves into Handl. The following remain in Rust as the residual host:

- Foreign `Resource` finalizers and the I/O/runtime resource table.
- The `Term` kernel data constructors and the `Language` data loader.
- The `Model` trait and the runtime registration/wrapping code.

Everything above this thin layer — value semantics, type semantics, bytecode and native compilers, optimizers, linters, formatters — is a Handl arrow loaded through the `Model` boundary.

## Consequences

### Positive

- New languages are data fragments, not host code changes.
- New engines are Handl modules, registered by name and loaded on demand.
- Self-hosting is staged: the compiler eventually compiles itself, but each stage has a fallback to the previous stage.
- Engine artifacts can be cached by `Term` hash, reducing startup and recompilation cost.
- The `Model` boundary isolates the host from engine evolution.

### Negative

- The bootstrap depends on a small number of Rust engines that must be trustworthy and correct.
- First-time compilation of a Handl compiler by an older Handl compiler is slow and must be carefully sequenced.
- `Op` is still a fixed host vocabulary; adding new primitives requires a host change.
- Foreign resource finalization and the `Term` kernel are privileged and cannot be overridden from Handl.

## Surface / registration examples

Kit runtime configuration:

```handl
kit/runtime {
    eval = "handl:engines.value_model"
}
```

A value engine module:

```handl
module engines/value_model

-- Evaluate a Term against an incoming Value in a ModelCtx.
op run : (Term, Value, ModelCtx) ~> Value
run(term, value, ctx) = ...
```

A compiler engine module:

```handl
module engines/cranelift_compiler

-- Compile a Term to a native Model artifact.
op run : (Term, ModelKind) ~> NativeArtifact
run(term, kind) = ...
```

## References

- ADR-0101 defines the frontier calculus `Term` variants and the `Model`/`Language` data design this ADR builds on.
- ADR-0102 demonstrates expressing new type constructs as data over the arrow calculus, the same pattern used here for engines.
