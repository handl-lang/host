---
title: 'Spike: hnl-run scale limit — where the 206 MB whisper-tiny-encoder .hnl dies'
type: report
date: 2026-11-15
---

# Spike: `hnl-run` scale limit — where the 206 MB artifact dies

Date: 2026-11-15 · Status: measurement spike · Artifact:
`/tmp/wt-enc.hnl` (206,222,180 B — `mlopt export arch:whisper-tiny-encoder`,
HEAD `4710f41` carrier-expand emit) · Runner:
`graphopt/target/release/hnl-run` → `mlopt_kit::CatoptKit::run` →
`handl_runtime::Runtime::run` = `lex → parse(+desugar) → eval`.

Probe used throughout: `graphopt/packages/mlopt-kit/src/bin/hnl-stage.rs`
(new scratch bin — runs `runtime.lex` / `parse_raw` / `apply_desugar` /
`run` stage-by-stage, printing elapsed + `VmRSS` after each). Host:
11 GB RAM, release build.

## TL;DR

- **It dies in `lex`**, before the parser sees a token. `hnl-stage` on
  the real file prints `kit-build` then nothing; RSS climbs ~140 MB/s
  and the process is OOM-killed at ~3.2–4.3 GB. Lex is **linear**
  (~8 src bytes/token, ~150–200 B/token RSS ⇒ ~18–22× source size), so
  206 MB ⇒ est. ~26 M tokens ⇒ ~4–4.5 GB of `Tokens` alone — over this
  box's available RAM.
- **No quadratic path found.** Every stage measured is linear in input
  size; the wall is a hard memory multiplier stack, and lex is simply
  the first stage to fall.
- **Secondary cliff (unrelated to this artifact):** the Pratt parser
  recurses per nesting level and **stack-overflows at ~9,500 paren
  depth** (main-thread 8 MB stack). `wt-enc.hnl` max depth is 119 —
  not its problem, but a real limit for generated/pathological input.
- **The real fix is emit-side, and it is already half-built:** mlopt
  terms are *hash-consed DAGs* (`mlopt_core.ir.Op.make` interns by
  structure), but `emit()` re-renders every occurrence as text. On the
  artifact's 43.4 MB body expression a streaming hash-cons count gives
  **258 unique subterms vs 3,998,256 tree occurrences (~15,500×)**.
  `.hnl` already has the needed mechanism — `name :: expr` bindings in
  `do {}` evaluate in order and later expressions reference earlier
  names (the leaf-binding channel emit already uses). Emitting shared
  subterms as `__sN :: tensor.…` collapses the compute body to ~10–20 KB.
- **…but that alone does not rescue this file.** ~162.5 MB (79%) of the
  artifact is `tensor.from_list([f64,…])` weight literal text inside the
  67 leaf bindings — irreducible in current `.hnl` (no binary/compact
  tensor literal exists; `tensor.*` has `from_list` and nothing else).
  Even a perfectly hash-consed 45 MB artifact still dies in lex on an
  11 GB box. Weights need an out-of-band channel (sidecar binary +
  `tensor.load`-style op, or a compact string encoding).

## 1. Where it dies — stage attribution

`hnl-run file.hnl` → `CatoptKit::run(src)` → `Runtime::run`
(`host/crates/handl-runtime/src/lib.rs:1052`):

```rust
let toks = self.lex(src)?;    // line 1053
let ast = self.parse(&toks)?; // line 1054  (parse_raw + desugar)
self.eval(&ast)               // line 1055
```

`hnl-stage /tmp/wt-enc.hnl` output:

```
src bytes=206222180 rss=204656 kB
[kit-build] 48.0ns rss=212292 kB
<killed — no [lex …] line ever printed>
```

RSS sampled every 0.3–0.5 s during a plain `hnl-run` run: monotonic
climb ~70 MB per 0.5 s sample (~140 MB/s) from ~205 MB to **4.35 GB
peak**, then the process dies (exit 143/SIGTERM-class kill on this
host; the carrier-expand note recorded -9 SIGKILL). Nothing else was
running; the kill is the OOM path. The lexer never returned.

### The memory-multiplier stack (all confirmed in source)

Even if lex fit, the pipeline multiplies memory at every stage:

| stage | what it costs | evidence |
|---|---|---|
| `Source` | the 206 MB `String` itself | trivial |
| `lex` → `Tokens` | `Token { kind: String, text: String, start, end }` — **two heap `String`s per token** (`kernel/crates/handl-language/src/staged.rs:22-36`) | measured below |
| `parse_raw` | `ParserRequest::new(toks.clone())` — **deep-clones every token** (`handl-runtime/src/lib.rs:841`) — tokens stay alive through `eval` too (`run` scope) | source |
| Pratt parse | `expression_inner` recurses per prefix child / infix rhs (`tool-parse-pratt/src/lib.rs:962`, child recursion `:1076`) | stack-overflow data below |
| parse return | `Ok(self.ast.borrow().clone())` — **clones the whole AST arena** (`tool-parse-pratt/src/lib.rs:810`) | source |
| `apply_desugar` | `ast.clone()` + `RewriteEngine::run_pass` → a second arena (`desugar_pass.rs:54-66`) | source |
| `eval` | `prepare_exec_artifact` (exec-erasure, another arena + lint traversal) → `lower_ast` → `Term` tree → `interpret_with_handlers`, falling back to tree-walk (`handl-runtime/src/lib.rs:898-919`) | source |

`AstNode` is `kind: String` + `span` + `payload: serde_json::Value`
(`handl-language/src/ast.rs:189-196`) — every literal/identifier is a
JSON value; an arena of ~10–15 M nodes is multiple GB, and it is built
~3 times (raw → desugar → erasure).

Estimated peak for `wt-enc.hnl` *if* lex survived: tokens ×2 (clone)
≈ 9 GB + ~3 AST arenas + Term + eval values ⇒ **~15–25 GB**. It is a
hard wall at every stage, not one bad stage.

## 2. Measured scaling (release `hnl-stage`, this host)

### lex — linear, ~18–22× source

Truncated prefixes of `wt-enc.hnl` (lex only; parse then fails on the
truncated tail, which is fine — the `[lex]` line is what matters):

| src bytes | tokens | lex time | RSS after lex |
|---:|---:|---:|---:|
| 5,000,000 | 617,665 | 0.50 s | 92 MB |
| 10,000,000 | 1,234,426 | 0.98 s | 174 MB |
| 20,000,000 | 2,750,690 | 2.1 s | 373 MB |
| 40,000,000 | 5,188,489 | 4.1 s | 698 MB |

≈ 7.7–8.1 src bytes/token, ≈ 130–145 B/token RSS, ~10 MB/s throughput.
Extrapolate 206 MB → ~26 M tokens → **~3.7–4.5 GB** — matches the
observed 3.2–4.3 GB death during lex.

### Synthetic end-to-end knees

`/home/luis/tmp/scale/` — `list-N.hnl` = `export f := (x) ~> [0.123456789, …]`
(N floats), `deep-D.hnl`/`plus-D.hnl` = N-deep paren/`+` nesting.

| file | src | lex | parse_raw+desugar | eval | verdict |
|---|---:|---|---|---|---|
| list-100k | 1.2 MB | 200k tok, 36 MB | 87 MB | **450 MB peak**, 1.1 s | OK |
| list-500k | 6.0 MB | 1.0M tok, 141 MB | 376 MB | **1,992 MB peak**, 5.4 s | OK |
| list-1M | 12 MB | 2.0M tok, 272 MB | 741 MB | killed at **~2.8 GB** in eval | **dies in eval** |
| list-2M | 24 MB | 4.0M tok, 533 MB | 1,378 MB | — | dies earlier still |
| deep-9000 | 18 KB | — | OK (43 MB) | OK (130 MB) | OK |
| deep-9500 | 19 KB | — | **stack overflow in `parse_raw`** | — | SIGABRT |
| deep-10000+ | — | — | stack overflow | — | SIGABRT |

So on an 11 GB box the **end-to-end knee is ~6–12 MB of source** for a
value-producing program (eval is the biggest multiplier — retained
tokens + ~2 AST arenas + Term + `serde_json::Value`/`TaggedValue`
materialization), and ~250–300 MB for lex alone. `wt-enc.hnl` at 206 MB
never reaches parse.

### Stack-overflow cliff (secondary)

`(`-nesting depth knee is **between 9,000 and 9,500** (8 MB main-thread
stack, ~840 B/frame in `expression_inner`). `deep-9000` parses, desugars
*and evals* fine; `deep-9500` aborts inside `parse_raw`. Eval's own
recursion survived 9,000. Cheap fix when wanted: run parse/eval on a
`std::thread` with a large `stack_size`, or make `expression_inner`
iterative for the prefix-child tail.

## 3. The emit-side finding — the term is a DAG, the artifact is a tree

`mlopt_core.ir.Op` is **hash-consed**: `Op.make` interns by
`(op, args, attr-key)` in a `WeakValueDictionary`
(`eqsat/packages/mlopt-core/src/mlopt_core/ir.py:188-283`) — structurally
identical subterms are literally the same Python object, and parents
share them freely.

`mlopt/src/mlopt/emit.py` walks that DAG with a plain recursive `go`
(`emit()`, ~line 1719) and re-renders **every occurrence**. The only
bindings emitted are `Var`/`Param` leaves via `name :: expr` lines in
the `do {}` prelude (`_prelude`, ~line 1834; `emit_program`/`emit_function`
~1868/1902). Internal sharing is never exploited.

Measured on the artifact itself — I wrote a streaming tokenizer +
hash-cons parser for the emitted call/list/number/name grammar and ran
it over line 71 (the entire 43,424,637-byte body expression):

- **258 unique subterms**
- **3,998,256 tree occurrences** (~15,500× mean expansion)
- `tensor.conv2d(tensor.unsqueeze(input_features, 2)` prefix occurs
  **82,944×**; `tensor.from_list([1.0])` 69,748×; `tensor.from_list([0.5])`
  64,093×; `tensor.from_list([1.4142135381698608])` 64,093×
- File composition: weight `::` bindings ≈ **162.5 MB (79%)**,
  compute body 43.4 MB (21%), everything else ~0.3 MB.

So the encoder's compute graph is tiny as a DAG (~hundreds of nodes —
weights are leaves bound by name) but exponential as a tree.

**The `.hnl` mechanism already exists.** `name :: expr` inside
`do {}` is a sequential let: each binding is evaluated in order and
later expressions — including other bindings' RHS — reference earlier
names (emit.py's own docstring relies on this for staged `{"hnl"}`
feeds). Emitting shared subterms as generated bindings is therefore a
pure emit.py change: post-order traversal, intern each `Op` by identity
(hash-consed ⇒ `id()`/the object itself is a perfect key), give every
node with refcount > 1 a `__sN ::` binding, render children as names.
Expected result for this artifact: body 43 MB → **~10–20 KB** of
bindings, and eval drops from ~4 M op dispatches (each shared subtree
recomputed per occurrence — conv1 alone 83 k times!) to ~hundreds.

**Honest caveat:** this shrinks 206 MB → ~163 MB. The remaining bulk is
weight literal text — ~10.3 M float literals at ~19 chars each inside
67 `tensor.from_list([...])` bindings. `host-tensor`'s op surface has
`from_list`/`to_list`/`zeros`/`ones`/`shape` and arithmetic — **no
binary, base64, or file-load channel** (`host-tensor/src/ops/mod.rs:66+`).
`.hnl` does have string literals (the `from "host-tensor"` import
spelling), so a `tensor.from_base64`/`tensor.load` op plus a compact
encoding is feasible without new literal syntax — but it is new
surface area in `host-tensor` + emit.

## 4. Ranked fix options

1. **Emit hash-consing (do it — small, mlopt-side only).**
   `emit_program`/`emit_function`: count parents per term node
   (one DAG walk), emit `__sN :: <expr>` for every node with >1 parent
   in post-order before the body, render shared children as names.
   Collapses the compute DAG; also fixes eval time (no recomputation).
   Does *not* fix weight bytes — necessary but not sufficient.
2. **Weights out-of-band (the real scale fix).** Sidecar binary
   (safetensors/npz/raw f64) + a `tensor.load(path)`/`tensor.from_bytes`
   host op, or a base64-string → `tensor.from_base64` op. Artifact
   becomes ~200 KB text + ~160 MB f64 / ~80 MB f32 binary (whisper
   weights are f32-origin; f32 round-trip may already satisfy the
   verify gate's tolerance — check before claiming). Alternative:
   shorten float spellings (f32-precision repr ≈ halves literal bytes)
   — still ~80–100 MB text, only a partial fix.
3. **Engine memory diet (host/kernel repos — bigger, general).**
   `Token.kind` → interned id/`Rc<str>`; `Token.text` → `Rc<str>` or
   source slice; kill `toks.clone()` in `ParserRequest` (borrow or
   `Rc<Tokens>`); kill `ast.borrow().clone()`; make desugar/erasure
   arena-reusing or in-place. Plausible 5–10× — takes the wall from
   ~250 MB to ~1–2 GB of *source*, i.e. fixes medium artifacts, not
   this one even after fix 1+2 unless weights are binary.
4. **Parse/eval on a big-stack thread** (cheap): `stack_size(64 MB)`
   removes the ~9.5 k-depth cliff. Independent of the memory wall.
5. **Streaming lexer→parser** (largest engine change): fuse lex+parse
   so tokens never materialize; pairs with 3 if binary weights land.

Note `hnl run`/`hnl eval` (`hnl-ext-eval`) uses the same data lexer +
Pratt parser + tree-walk, so it hits the identical wall — fixes in the
engines help both doors.

## Files / commands

- Read: `graphopt/packages/mlopt-kit/src/{lib.rs,bin/hnl-run.rs}`,
  `host/crates/handl-runtime/src/lib.rs` (`run`/`lex`/`parse`/`eval`,
  lines 810-1056), `desugar_pass.rs`, `exec_erasure_pass.rs`,
  `tool-parse-pratt/src/lib.rs` (`parse`/`expression_inner`),
  `handl-language/src/{staged.rs,ast.rs}`, `mlopt/src/mlopt/emit.py`,
  `eqsat/.../mlopt_core/ir.py`, `host-tensor/src/ops/mod.rs`.
- Wrote: `graphopt/packages/mlopt-kit/src/bin/hnl-stage.rs` (scratch
  stage-wise probe — left uncommitted in the tree; delete or keep as a
  diagnostic), `/home/luis/tmp/scale/{list,deep,plus}-*.hnl` +
  `dagsize.py` (synthetic scale inputs + the hash-cons analyzer).
- Ran: `cargo build --release -p mlopt-kit --bin hnl-stage`;
  `hnl-run`/`hnl-stage` under an RSS-sampling `/proc` monitor on
  `/tmp/wt-enc.hnl` and the synthetic files (table above).
- Not done: no engine or emit changes; the 206 MB artifact was not
  regenerated (weights/term not persisted — analysis ran on the emitted
  text itself, which is sufficient for the DAG count).
