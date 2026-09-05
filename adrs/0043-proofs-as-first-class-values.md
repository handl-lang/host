# ADR-0043: Proofs as First-Class Values

## Status

Proposed

Builds on [ADR-0042 — Transformations as First-Class Values](./0042-transformations-as-first-class-values.md).

## Context

ADR-0041 made `Language` a first-class value. ADR-0042 made
transformations `Language -> Language` first-class values. The next
operator up is the one that operates on transformations: **the claim that
a transformation is correct** — that `compile(L, "wasm")` preserves L's
semantics, that `optimize` is semantics-preserving, that a translator
`A -> B` faithfully translates.

Today that claim is a theorem you prove offline — in Coq, on paper, or
not at all. The proof lives outside the program. The runtime trusts the
toolchain because it cannot see the proof.

Once transformations are values, the proof "this transformation preserves
semantics" is a value too. The operator that worked on level N becomes a
value at level N+1, again:

| level | what's first-class | the operator                                 |
| ----- | ------------------ | -------------------------------------------- |
| 1     | functions          | `apply : (A -> B) -> A -> B`                 |
| 2     | languages          | `eval : Language -> Source -> Value`         |
| 3     | transformations    | `compose : (L -> L) -> (L -> L) -> (L -> L)` |
| 4     | proofs             | `verify : Proof -> Bool`                     |

A proof stops being a paper artifact and becomes a value you bind, pass,
check, and compose. Compiler correctness stops being "trust the
toolchain" and becomes "verify the value."

### Why this is proof-carrying code, at the language level

Proof-carrying code (Necula, 1997) attaches a proof to compiled code so a
verifier can check safety before running it. PCC is usually framed at the
instruction level — a proof that this binary doesn't violate memory
safety. This ADR lifts the same idea to the language level: a proof that
this `Language -> Language` transformation preserves semantics. The
compiled artifact ships with its correctness proof as a value; the
runtime checks the proof before applying the transformation.

The difference from classical PCC: the proof is a first-class handl
value, not an external certificate. It can be composed, transformed,
and reasoned about in-program, the same way languages and
transformations can.

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

## Consequences

### Positive

- **Compiler correctness becomes a runtime check.** A compiler ships
  with its proof; the runtime verifies before running. No trust in the
  toolchain, only trust in the small fixed verifier.
- **Proofs compose with transformations.** A pipeline's proof is the
  composition of its passes' proofs. Verify once, not per pass.
- **Certified compilation.** A compiler is a `(transformation, proof)`
  pair. This is proof-carrying code at the language level.
- **Verified interop.** A translator ships with a faithfulness proof.
  Polyglot interop is verified, not trusted.
- **The tower is uniform.** Proofs are values in a language,
  transformations are functions in a language, languages are evaluated
  by `eval`, functions are applied by `apply`. No privileged
  meta-level. Level 4 bottoms out at level 1, the way level 3 did at
  level 2.
- **Auditable supply chain.** A language or compiler shipped as a value
  carries its proof of correctness. The consumer verifies before use.
  No more "trust the binary."

### Negative

- **Proof construction is hard.** Generating proofs for nontrivial
  transformations is the open problem of automated theorem proving.
  Interactive theorem provers (Coq, Lean) work but require human
  effort. SMT-based approaches scale but only for restricted property
  languages. This ADR does not solve proof construction; it makes
  proofs _first-class_ so that whatever construction method is used
  (manual, automated, or hybrid) produces a value the runtime can
  check.
- **The verifier must be trusted.** The runtime checks proofs against a
  verifier. That verifier is now the trusted computing base, the way a
  typechecker is. It must be small, fixed, and audited. This is the
  same trust reduction as proof-carrying code: trust a small verifier
  instead of a large toolchain.
- **Compositional correctness is not free.** Proof composition requires
  that the notion of correctness is compositional. Semantics
  preservation is (under conditions), but not every property is. The
  framework must track which proofs compose and which do not.
- **Nobody has shipped this in a production language.** Proof-carrying
  code exists at the instruction level (Necula, 1997; Typed Assembly
  Language). Certified compilation exists in research compilers (CompCert,
  CakeML). First-class proofs as runtime values in a general-purpose
  language is uncharted. This is the research frontier, one level above
  the research frontier of ADR-0041.
- **Performance.** Proof verification at load time has a cost. For
  large proofs (which nontrivial transformations produce), this is
  non-trivial. Caching verified proofs mitigates, but the first load
  pays.

## The full tower

| Level | What's first-class | The operator | What it unlocks                             |
| ----- | ------------------ | ------------ | ------------------------------------------- |
| 0     | Data               | —            | Values                                      |
| 1     | Functions          | `apply`      | Recursion, Turing completeness              |
| 2     | Languages          | `eval`       | Universal host, self-bootstrap              |
| 3     | Transformations    | `compose`    | Composable compilers, category of languages |
| 4     | Proofs             | `verify`     | Certified compilation, verified interop     |

Each level makes the operator of the previous level a value. Each level
is the same move. Each level bottoms out at the one below — proofs are
values in a language, transformations are functions in a language,
languages are evaluated by `eval`, functions are applied by `apply`. The
tower is uniform; there is no privileged meta-level.

## Implementation phases

### Phase 1 — Define the proof value type

A `Proof` is a handl value attesting to a property of a transformation.
The exact encoding (dependent types, refinement types, explicit
certificate) is deferred to implementation. The contract is: `verify :
Proof -> Bool` is a small, fixed, audited function.

### Phase 2 — Verify before apply

The runtime checks `verify(proof)` before applying a certified
transformation. Unverified transformations are allowed but flagged
(unlike ADR-0041's `eval`, `apply` of a transformation _can_ be gated,
because transformations are not plumbing — they are the thing with
authority over the result language's semantics).

### Phase 3 — Proof composition

`compose-proofs : Proof -> Proof -> Proof` mirrors `compose` for
transformations, for compositional properties (semantics preservation).

### Phase 4 — Certified compilers and translators

Ship `(transformation, proof)` pairs for the standard transformations
(`compile`, `optimize`, `restrict`, translators). The runtime verifies
before applying.

### Phase 5 — Proof construction tooling

Tooling to construct proofs — interactive, SMT-based, or hybrid. This is
the hardest phase and the most open. The ADR does not mandate a specific
method; it mandates that whatever method is used produces a first-class
`Proof` value the runtime can check.

## Related work

- [ADR-0041](./0041-languages-as-first-class-values.md) — Languages as
  First-Class Values
- [ADR-0042](./0042-transformations-as-first-class-values.md) —
  Transformations as First-Class Values (this ADR certifies those
  transformations)
- Proof-carrying code (Necula, 1997) — proofs attached to compiled code,
  verified before execution; this ADR lifts the idea to the language
  level
- CompCert — certified C compiler (Leroy et al.); offline proof, not a
  runtime value
- CakeML — verified ML compiler; offline proof, not a runtime value
- Typed Assembly Language — types as proofs at the instruction level
- Refinement types (LiquidHaskell, F*) — proofs as types, in-language
- 3-Lisp — reflective tower; this ADR is the proof level of the tower
