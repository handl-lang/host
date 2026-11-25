+++
id = "PL-001"
title = "Cubical equality migration (ADR-0208)"
status = "open"
created = "2026-11-24"
owner = "devin"
priority = "high"
links = ["ADR-0208"]
+++

Migrate Handl's equality foundation from intensional MLTT (`Path{frm,to}` + substitution transport + primitive `J`) to cubical type theory (CCHM-style): interval `I`, `Path`/`PathP` as interval functions, `transp`/`hcomp`/`comp`, `Glue`/`unglue`/`isFibrant`, computing univalence. Decision record: `adrs/0208-adr-0208-cubical-equality-interval-glue-and-computing-univalence.md`.

Design decisions (fixed): `I` sort with `i0`/`i1` + dim vars; faces `Φ` = `i=0`,`i=1`,`~i`,`i∨j`,`i∧j`,`0`,`1`; `Path A x y := (i:I)→A`, `PathP A x y := (i:I)→A i`; `refl x := λi.x`; `J` removed or demoted to derived; types erased at runtime so the interval lives in the type theory only. Canonical impl: `tool-typecheck-handl/src/models/typecheck_model.hnl` (+2 fixture copies, re-sync via `cp`); production parity in `tool-typecheck-nbe` (Rust). Engine genericity: YAML data + .hnl only, no kind hardcoding.

## Phases

- [x] **P0 design lock** — plan + ADR-0208 + implementation map
- [x] **P1 interval + Path/PathP** — `type.interval`, `i0`/`i1`, path-carrying `type.pi{param:interval, path:{x,y}}`, `synth_path_type_`, dual witness acceptance. Commits: tool-typecheck-handl `f72e304`, host `a697fb8a`; NBE parity `64d77de`+`ada56882`.
- [x] **P2 transp/hcomp/comp/trans/symm + faces Φ** — model `1903399`, host yaml `20aeed04`; NBE parity `45b0f9a`.
- [x] **P3 Glue/unglue/isFibrant/fibrant** — model `694f512`, host `60bbc5bc`; NBE parity `0c98f83`.
- [x] **P4 computing univalence** — `ua`/`univalence`/`Univalence`/`Equiv` via recorded-equivalence path-pi; `transp (ua e)` applies domain→codomain incl. dependent families. Model `79dceb9`, host `1911c05a`; NBE parity `3d1197b` (491 tests green) — univalence computes in production.
- [ ] **P5 elimination rework** — model changes on disk (uncommitted): `j_motive` interval family in `apply_dep_closure_`, representation-agnostic `witness_from_type_`/`paths_in_scope_`, PathP transport branch in `synth_transport_`, `synth_j_` rebuilt as derived J. `tests/cubical_p5.rs` (7 tests): 3 pass, 3 real failures under repair (agent `92982217`), 1 pending rerun. YAML `type.j`/`eval.j` preserved until parity, then final excision.
- [x] **P6a paper** — `kernel` `eb1b6f3`: "Equality as interval data" section + CCHM/Huber/HoTT citations; post-P5 wording spots flagged.
- [x] **P6b smoke** — host `d742b6ef`: `tests/smoke/cubical{,_err}.hnl`, suite 24/24 green through production `hnl typecheck`.
- [ ] **Final cleanup** — `j`/`type.j`/`eval.j`/`JElim` excision once P5 parity lands; univalence smoke cases; paper post-P5 wording flip; fixture-drift test for the three `typecheck_model.hnl` copies.
- [ ] **dependent_structure.rs repair** — 14 pre-existing failures from retired `data` keyword; rewrite to family-record spelling (`Bool := { True : Bool, … }`) is on disk uncommitted, awaiting verification after P5 model lands.

## Production parity gaps found by smoke — ALL RESOLVED

Fixed by agent `232de29c`: NBE `dd1ccfe` + host `13c9dff3` (cubical vocab in `prelude_wire_names()`, extended smoke). Suite at 24/24 green.

1. ~~`i0`/`i1` literals rejected as `transp`/`hcomp`/`Comp` dimension operands~~ — fixed.
2. ~~`p # v` annotated over homogeneous term endpoints fails~~ — fixed.
3. ~~`glue A, φ, T̂` keyword former unusable~~ — fixed (`glue`/`Pi`/`I`/`Path`/`Equiv`/`ua` unbound-wire resolution).
4. ~~Univalence smoke pending~~ — univalence smoke added; computes in production.

## Design insights (recorded from design discussion — future work candidates)

The everything-is-data architecture compounds beyond the type theory itself:

- **One formalism, three roles** — program, proof, and rewrite rule are all cells/terms in the same calculus; laws are executable data.
- **Shared compute across the typecheck/runtime boundary** — types and terms share the same `Term` evaluator (`eval.yaml` schemes are data). Typechecking already *runs* computations (normalizing `Vec(2,Bool)` evaluates indices); since the graphs are identical, the checker's normalization cache can seed runtime evaluation — the typechecker acts as a demand-driven partial evaluator whose cache the runtime inherits. TS/Coq can't do this: their checkers and runtimes are different machines.
- **Proofs as replayable rewrite scripts** — `transport_term_`/`subst_index_` substitution along a `Path` witness is the same rewrite a compiler performs; the witness IS the script.
- **Grades mark the cache boundary** — `~[effects]~>` rows type exactly which computations are pure; pure graded arrows are safe to share across the typecheck/runtime cache boundary, effects stay effectful. Purity is *checked*, not guessed.
- **Judgments as cacheable/parallelizable data** — `synth` results are values: memoize `(node, env) → type`, shard unification/match-arm checking across workers, ship precomputed signatures between repos. Serializability makes distributed typechecking a data problem, not a systems problem.
- **Caveats**: normalization must terminate (same totality question as `loop`/`rec`); proof-irrelevant parts erase anyway — but surviving index computations and type families are exactly the expensive pure work worth caching.

## Known issues / follow-ups

- `dependent_structure.rs`: 14 pre-existing failures (`unbound variable: data` — retired keyword), unrelated.
- `typecheck_model.hnl` fixture drift — no sync mechanism; phases re-sync by `cp`, consider a drift test.
- Disk pressure on this machine; always `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_*_DEBUG=0`, `-j2`, `CARGO_LOCK_HELD=1 /usr/bin/cargo` to bypass the flock queue.
- `sect`/`retr` equivalence proof fields deferred (proof-irrelevant, ADR-0151); `to`/`fro` are provenance only.
- Glue partial-section checking approximate; fibrancy of `Type`/universe resizing unaddressed.
