# ADR-0125: A Rust bootstrap **type synthesiser** driven by `lang.types`

## Context

`Kit::typecheck(input)` returns a **synthesized type** (a `Value`; types are
values, ADR-0032). Three facts make that hard to serve today.

### 1. The only engine that produces a type is the interpreted model

`plugin-engine-typecheck-handl`'s `HandlTypecheckEngine` loads
`project/design/adr0103/typecheck_model.hnl` (~51k chars) and runs it in the
tree-walk interpreter. It is the only registered typecheck engine
(`build_kit` registers `"handl"`), so *every* `kit.runtime().typecheck()` goes
through it.

Measured cost (debug): a **3-node** AST synthesises a type in **7.65 s**; a
**565-node** AST (a desugared `data Nat`) burns its 50k fuel budget for
**175 s** and then reports `typecheck fuel exhausted`. Inside the call,
`ensure_model` is 305 ns and `marshal_language` is 6.2 ms — the entire remaining
cost is the model's own `func.call`. The code already says as much:

> measured at minutes even for a 9-node AST, so it is unusable as a default CLI
> path

with a `MAX_MODEL_NODES = 1000` circuit breaker to skip the model on larger
ASTs. In practice the breaker is far too generous and the model is slow even
where it succeeds.

### 2. The bootstrap checker cannot produce a type

`plugin-engine-typecheck`'s `TypecheckReport` has exactly one field:

```rust
pub struct TypecheckReport { pub diagnostics: Vec<Diagnostic> }
```

No `typ`. It performs the scope/unbound-wire pass and nothing else. So the fast
path cannot satisfy `Kit::typecheck`, and the existing `NotImplemented`
fallback in `typecheck_handl_with_prelude` returns `Value::Null`, not a type —
which is why ~20 unit tests (`kit.typecheck("42")` -> `type.number`, plus
`type.bool`, function types, `do { x :: 1; x }`, `match`, annotation mismatches)
pin the model in place.

### 3. The rules already exist as data — with no interpreter

`lang.types` holds **82 `TypeRuleDef`s** merged from
`handl-kit-data-canonical/data/types.yaml` (ADR-0054, "Generic Typecheck
Engine"). Each declares:

```rust
pub struct TypeRuleDef {
    pub id: String,          // "type.if"
    pub node_kind: String,   // "if"
    pub synth: Term,         // Γ ⊢ node ⇒ T  (computes the type value)
    pub check: Option<Term>, // Γ ⊢ node ⇐ T  (directed checking)
    ...
}
```

The schemes are written in the same arrow-calculus `Term` kernel the evaluator
uses. **Nothing in Rust reads `synth`** — a workspace-wide grep finds
`lang.types` only in `merge` calls and test assertions. The engine that used to
evaluate them (`plugin-engine-typecheck-nbe`) was deleted; its judgment ops are
gone with it.

Measured scope of the data: the 82 rules use **40 distinct judgment ops, 469
times**, and a core of six covers 85% of uses.

| uses | op |
| ---: | --- |
| 110 | `construct` |
| 78 | `result` |
| 65 | `bind` |
| 64 | `synth_child` |
| 47 | `unify` |
| 35 | `check_child` |

The remaining 34 ops each appear 1-14 times (`field`, `type_field`,
`payload_field`, `make_array`, `node_payload`, `synth_object_record`,
`construct_pi`/`sigma`/`forall`, `synth_match_arms`, `synth_rec`, `synth_data`,
`synth_flow`, `synth_axis_flow`, `synth_transport`, …).

`plugin-engine/src/term_interpreter.rs` (624 lines) already evaluates `Term`s
op-by-op (`interpret_with_handlers` dispatches on `Term::Op { name, args, lit,
source }`, e.g. `"record"`, `"field_access"`), but it has **no judgment ops** —
so `source: Judgment` terms are inert today.

## Decision

**Implement the synthesizer as a Rust `TypecheckEngine` that drives
`lang.types` — and make it the default.**

1. **Implement the judgment ops** as ops in the term interpreter (or a
   dedicated judgment registry alongside it), covering all 40 names in
   `types.yaml`. They fall into three groups: term/value plumbing
   (`result`, `bind`, `node_payload`, `field`, `payload_field`, `make_array`,
   `json_type_construct`), traversal (`synth_child`, `check_child`,
   `synth_last_child`, `check_all_children`, `proj_child`, …), and the
   type theory itself (`construct`, `unify`, `fresh_meta`, `solve_meta`,
   `quote`, `normalize`, `construct_pi`/`sigma`/`forall`, `unify_pi`, …).

2. **A thin engine drives it**: for each AST node, look up the `TypeRuleDef`
   whose `node_kind` matches, evaluate its `synth` term, return the type. When
   a kind has no rule, return `type.unknown` rather than failing — the model
   behaves the same way.

3. **Gate the semantics on parity, not on trust.** A test asserts the
   bootstrap synthesizer and the interpreted model agree on the same corpus.
   Disagreement is a bug in the new ops, not a licence to change expectations.

4. **Flip the default** once parity holds: register the bootstrap engine as
   the runtime typecheck engine and keep the interpreted model reachable
   explicilty via `Kit::typecheck_handl*` (which already owns the
   model↔bootstrap fallback). Then tighten `MAX_MODEL_NODES` from 1000 to the
   measured cliff, and give `Kit::typecheck` a real type instead of
   `Value::Null` on the fallback path.

5. **Fix the config**: `data/kits/canonical.yaml` says `runtime.typecheck: nbe`,
   naming an engine that no longer exists, while `build_kit` silently overrides
   it with `"handl"`. Pick the real name so the data stops lying.

## Consequences

**Positive.**

- The last slow tests stop being slow. They are slow only because the model is
  the default; with the bootstrap path they are microseconds.
- `Kit::typecheck` gets a *real* synthesized type on its fast path, so the ~20
  assertions that pin the model become satisfiable without it.
- **No duplicated rules.** The synthesizer reads `types.yaml`, so a new rule
  written as data starts working with no Rust change. That is the ADR-0054
  premise, restored.
- The two engines become genuinely comparable: a parity test between a fast
  Rust engine and the interpreted model is a much stronger correctness signal
  than either alone.
- `hnl typecheck` and the LSP stop paying a 40 s / 175 s class of cost.

**Negative / constraints.**

- 40 ops is real work: the type-theory group (`unify`, `fresh_meta`,
  `solve_meta`, `normalize`, `quote`, the Π/Σ/∀ constructors) is a
  normalization/unification engine, not a lookup table. This is the substance
  of the ADR, and the long tail (34 ops used 1-14 times) is where the effort
  actually goes.
- Parity must be maintained deliberately: keeping the model runnable is what
  makes the parity test possible, so the model cannot be deleted as part of
  this work even though it stops being the default.
- Until parity is green the default must not flip — otherwise a divergent
  synthesizer silently changes what `hnl typecheck` reports.

## Alternatives rejected

- **Hand-write Rust for the ~20 shapes the tests assert.** Fastest to land, but
  it duplicates rules that already exist as data and drifts as the language
  grows; it would silently disagree with `types.yaml` on the other ~60 rules.
- **Keep the model as the default and only tighten `MAX_MODEL_NODES`.** Cheap,
  but it does not produce a type where the breaker fires — it returns
  `NotImplemented`, and the fallback yields `Value::Null` — and it leaves
  `hnl typecheck` slow for every AST under the cap, i.e. most of them.
- **Delete the model now that it is not the default.** Removes the only
  independent oracle, so the parity test — the thing that makes this safe —
  becomes impossible. Keep it as the opt-in research path (ADR-0103/0105).
- **Make the model fast instead (compiled eval engine).** Orthogonal and worth
  doing eventually, but a much larger change to the eval substrate, and it
  would still leave the type logic in an interpreted `.hnl` model rather than
  in the language data that already declares it.

## Migration

1. **Write the ADR** (this document) and a task file naming the ops and the
   parity corpus.
2. **Implement the six core ops** (`construct`, `result`, `bind`,
   `synth_child`, `unify`, `check_child`) plus the engine loop for a minimal
   node set — literals, `var`, `if`, `function`, `do`-block.
3. **Parity test** against the model on that set; iterate until green.
4. **Implement the tail** (the other 34 ops), extending parity to the full
   corpus — the `fp_integration` cases and the `kit.typecheck` unit assertions
   are the natural corpus.
5. **Flip the default**; tighten `MAX_MODEL_NODES` to the measured cliff; give
   `Kit::typecheck`'s fallback a real type.
6. **Fix `canonical.yaml`** to name the engine that actually exists.
7. Re-run the full suite and record the before/after wall clock.

## Status

**Proposed.** Not implemented. Blocked on nothing: the rules, the op
vocabulary, and the term interpreter all exist — what is missing is the ops and
the loop. Scope is measured above rather than estimated: **40 ops, 469 uses,
6 core ops covering 85%**.

Open question for review: whether the judgment ops belong in
`plugin-engine/src/term_interpreter.rs` (shared with the evaluator, but mixes
two op vocabularies in one dispatch) or in a dedicated interpreter in
`plugin-engine-typecheck` (clearer ownership, a little duplicated traversal).
Recommendation: the dedicated interpreter, reusing `Term` and the traversal
helpers, so the judgment vocabulary stays in the crate that owns the rules.

