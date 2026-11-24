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
- [x] **P2 transp/hcomp/comp/trans/symm + faces Φ** — model `1903399`, host yaml `20aeed04`; NBE parity in flight.
- [ ] **P3 Glue/unglue/isFibrant** — in flight: real type formers for existing `glue`/`fibrant` keywords.
- [ ] **P4 computing univalence** — `ua : (A≃B) → Path A B` via Glue; `transp (ua e) ≡ e`.
- [ ] **P5 elimination rework** — PathP-based dependent matching; `eq_*` witnesses → interval paths; `j` derived or removed (excise `synth_j_`, `type.j`, `eval.j`, `JElim`).
- [ ] **NBE parity trail** — each phase needs the Rust port (P1 `64d77de` done, P2 in flight, P3+ pending).
- [ ] **P6 docs/paper** — paper.qmd equality section, bibliography (CCHM 2018, Huber 2016), smoke coverage once NBE supports forms.

## Known issues / follow-ups

- `dependent_structure.rs`: 14 pre-existing failures (`unbound variable: data` — retired keyword), unrelated.
- `typecheck_model.hnl` fixture drift — no sync mechanism; phases re-sync by `cp`, consider a drift test.
- Disk pressure on this machine; always `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_*_DEBUG=0`, `-j2`.
- `univalence`/`glue`/`fibrant` had grammar/ast/tokens but no type rules — P3 fills glue/fibrant, P4 univalence.
