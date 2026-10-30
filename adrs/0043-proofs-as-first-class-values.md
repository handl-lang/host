---
title: Proofs as First-Class Values
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Builds on [ADR-0042 — Transformations as First-Class Values](./0042-transformations-as-first-class-values.md).
---


# ADR-0043: Proofs as First-Class Values

## Decision


Make proofs first-class handl values. A proof is a value that attests to
a property of a transformation. The runtime verifies proofs before
applying the transformations they certify.

### 1. Proofs are handl values

```handl
# a proof is a value: Transformation -> Proof
correct: type.any = (t: type.any) -> prove-preserves-semantics(t);

# prove that a specific compiler is semantics-preserving
wasm-compiler: type.any = to-wasm(handl);
wasm-proof:    type.any = correct(wasm-compiler);    # a proof value
```

The proof is constructed (offline, by a solver, or by a human) and then
carried as a value alongside the transformation it certifies.

### 2. Verification is a runtime check

```handl
# compiler correctness becomes a runtime check, not a paper theorem
safe-compile: type.any = (l: type.any) -> {
  t: type.any = to-wasm(l);
  p: type.any = correct(t);
  if verify(p) then apply(t, l) else reject
};
```

The runtime checks the proof value before applying the transformation.
No trust in the toolchain; only trust in the verifier (which is small
and fixed, the way a typechecker is small and fixed).

### 3. Proofs compose with transformations

If `p1` proves `t1` correct and `p2` proves `t2` correct, then
`compose-proofs(p1, p2)` proves `compose(t1, t2)` correct — provided the
notion of correctness is compositional (semantics preservation is).

```handl
# compose transformations and their proofs together
pipeline: type.any = compose(optimize, strip-io);
proof:   type.any = compose-proofs(proof-optimize, proof-strip-io);

# the composed pipeline is certified by the composed proof
if verify(proof) then apply(pipeline, handl) else reject;
```

Proof composition mirrors transformation composition. A compiler
pipeline ships with a composed proof, and the runtime verifies the
composed proof once, not each pass individually.

### 4. Certified compilation

A compiler ships as a pair: `(transformation, proof)`. The runtime
verifies the proof before running the compiler. This is proof-carrying
code at the language level.

```handl
# a certified compiler is a value: (Transformation, Proof)
certified-wasm: type.any = {
  transform: to-wasm,
  proof:     correct(to-wasm),
};

# the runtime verifies before applying
run-certified: type.any = (cert: type.any, l: type.any) -> {
  if verify(cert.proof) then apply(cert.transform, l) else reject
};
```

### 5. Verified interop

A translator `A -> B` ships with a proof that it faithfully translates.
Polyglot interop stops being "trust the bridge" and becomes "verify the
translator."

```handl
json-to-handl-cert: type.any = {
  transform: json-to-handl,
  proof:     prove-faithful(json-to-handl),
};

# verified translation at runtime
if verify(json-to-handl-cert.proof) then
  eval(handl, translate(json-to-handl-cert.transform, json-src))
else
  reject;
```


## Why

ADR-0041 made `Language` a first-class value. ADR-0042 made transformations `Language -> Language` first-class values. The next operator up is the one that operates on transformations: **the claim that a transformation is correct** — that `compile(L, "wasm")` preserves L's semantics, that `optimize` is semantics-preserving, that a translator `A -> B` faithfully translates.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Compiler correctness becomes a runtime check.** A compiler ships
- **Proofs compose with transformations.** A pipeline's proof is the
- **Certified compilation.** A compiler is a `(transformation, proof)`
- **Verified interop.** A translator ships with a faithfulness proof.
- **The tower is uniform.** Proofs are values in a language,

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0043-proofs-as-first-class-values.md`](../design/0043-proofs-as-first-class-values.md).
