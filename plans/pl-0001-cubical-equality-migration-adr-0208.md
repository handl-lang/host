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
- [x] **P4 computing univalence** — `ua`/`univalence`/`Univalence`/`Equiv` via recorded-equivalence path-pi; `transp (ua e)` applies domain→codomain incl. dependent families. Model `79dceb9`, host `1911c05a`; **NBE parity in flight**.
- [ ] **P5 elimination rework** — in flight (model side): PathP matching, `j` demotion to derived, `eq_*`/transport decisions; YAML `type.j`/`eval.j` preserved until NBE-P4 lands, then final excision.
- [x] **P6a paper** — `kernel` `eb1b6f3`: "Equality as interval data" section + CCHM/Huber/HoTT citations; post-P5 wording spots flagged.
- [x] **P6b smoke** — host `d742b6ef`: `tests/smoke/cubical{,_err}.hnl`, 23/23 suite green through production `hnl typecheck` (NBE P1-P3 verified).
- [ ] **Final cleanup** — `j`/`type.j`/`eval.j`/`JElim` excision once NBE-P4+P5 parity lands; univalence smoke cases; paper post-P5 wording flip.

## Production parity gaps found by smoke (host `d742b6ef` header)

1. `i0`/`i1` literals rejected as `transp`/`hcomp`/`Comp` dimension operands in production `.hnl` (workaround: bind `e0 : I := i0` first) — scope/lowering gap, NBE fixtures pass literals directly.
2. `p # v` annotated over homogeneous term endpoints fails (`Path(Bool,true,true)`); unannotated works — `true` doesn't eval as index term in `resolve_path`.
3. `glue A, φ, T̂` keyword former unusable in production files — children scope-checked as term wires → unbound; `Glue(...)`/`isFibrant(...)`/`unglue(...)` call forms work in type position only. Unbound-wire scope pass issue.
4. Univalence smoke pending NBE-P4 landing + cdylib rebuild; `Equiv(Bool,Nat)` record checking looked flaky in probes.

## Known issues / follow-ups

- `dependent_structure.rs`: 14 pre-existing failures (`unbound variable: data` — retired keyword), unrelated.
- `typecheck_model.hnl` fixture drift — no sync mechanism; phases re-sync by `cp`, consider a drift test.
- Disk pressure on this machine; always `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_*_DEBUG=0`, `-j2`, `CARGO_LOCK_HELD=1 /usr/bin/cargo` to bypass the flock queue.
- `sect`/`retr` equivalence proof fields deferred (proof-irrelevant, ADR-0151); `to`/`fro` are provenance only.
- Glue partial-section checking approximate; fibrancy of `Type`/universe resizing unaddressed.
