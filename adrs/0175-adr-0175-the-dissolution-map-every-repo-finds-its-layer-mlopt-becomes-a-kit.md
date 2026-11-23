+++
id = "ADR-0175"
title = "ADR-0175: The dissolution map — every repo finds its layer, mlopt becomes a kit"
status = "done"
priority = "high"
+++

---
title: The dissolution map — every repo finds its layer
type: adr
date: 2026-11-21
status: accepted
status_note: "Accepted 2026-11-21 — user ratified interactively: game trio → meta/, P2 = engine-eqsat over kernel Term, mlopt dissolves after P0.5+P1, kernel-ir v2 deferred until compile-cuda/census demands it, opt-* taxonomy names abandoned (repos never existed — dissolution moves contents, then archives)."
applies_to: 'the whole handl-lang org — repo taxonomy, names, and dissolution sequencing'
---

# ADR-0175: The dissolution map

## Decision

The org's end-state is **six layers + one meta tier**, and every
current repo dissolves into them. Names are scaffolding; the
layers are the structure.

### The layers

```
KERNEL     kernel/  engine/  (handl-language, handl-value)
INTERPRET  engine-exec-*  engine-typecheck-*  engine-transform-*
           engine-lex/parse/fmt/lint  compile-*  (emitters)
HOST       host/  host-cli  host-lsp   (dumb platform + op registry)
KIT        kit-prelude  kit-prelude-lang  kit-data-prelude
           kit-mlopt  kit-torch  kit-onnx  kit-kernel  kit-eqsat
LIB        lib-tensor  (lib-* = .hnl/Rust libraries)
META       gamekit  discoverykit  benchkit   (self-improvement libs)
```

### The dissolution map (ratified)

| today | becomes | when |
|---|---|---|
| `host-tensor` | `lib-tensor` + tensor `Language` fragment data; wire codec → value-library spec | P1+ (needs fragment) |
| `host-ext-*` | **category dies** — extensions are kits registering commands/ops; `hnl-ext-mlopt` was 90 lines of argv forwarding | with mlopt dissolution |
| `lib-kernel-ir` | kernel fragment op vocabulary (ops-as-data); JSON wire stays as compile-* input | P1+ |
| `eqsat` | `engine-eqsat-*` (search interpretation over kernel `Term` — P2 decision: NOT the egraph crate; egraph stays an internal library) + `kit-eqsat` (laws/costs/minted.db as kit data) | P2–P3 |
| `graphopt` | contents split: `kit-torch` + `kit-onnx` (+ cuda sink kit); **the repo itself is renamed `mlopt`** — it already hosts `mlopt-discovery`/`mlopt-host-tensor`/`mlopt-onnx` and outsiders read the name as the product (user call); old `mlopt` repo merges in or archives first to free the name | P3 |
| `kernelopt` | `kit-kernel` (kernel laws as data) + compile-* emitters | P3 |
| `mlopt` | merges into renamed-graphopt→`mlopt` (the kit-mlopt home) — **after P0.5+P1 land** (user-ratified timing) | P3 |
| `gamekit`/`discoverykit`/`benchkit` | stay top-level under the **meta tier** — they ship *pipeline scripts* (`hnl run *.hnl`), not engines; see run layer below | names kept |

### The run layer (user-raised, ratified)

Pipelines are `Term`s: `lift >>> search >>> verify >>> deliver`
is `Seq` composition. `hnl run <script>.hnl` evaluates a script
whose stage-ops (`lift`, `search`, `verify`, `deliver`, `mint`,
`gauntlet`, `bench`) are registered by kits — gamekit/
discoverykit/benchkit provide the meta-tier ops. No new machinery:
`hnl run` is an engine interpretation; the game layer becomes
composable, user-writable scripts.

### Command surface (end state)

```
hnl optimize model.torch     # engine-eqsat interpretation
hnl compile  model.hnl --to cuda   # compile-* emitters
hnl run      program.hnl     # exec engines
hnl doctor                    # capability discovery
```

`mlopt` survives only as `kit-mlopt`'s registered adapter names
(`whisper:`, `kokoro:`, `.hnl` refs). Users never type "mlopt."

### Binding generation (noted, not scheduled)

Backend FFI needs no sdkgen where the backend ships its own
registry (torch: `torch.ops.aten` + `Declarations.yaml`). A
schema→bindings generator per backend family is the right
generalization when typed Python wrappers are wanted — a `lib/`
or `kit/` concern, deferred until a second backend needs it.

### On the abandoned `opt-*` taxonomy

The `eqsat→engine-eqsat`, `graphopt→opt-graph`, etc. rename list
was a proposal — **no `opt-*` repos ever existed**. Nothing to
delete. Dissolution moves *contents* into the layer homes; source
repos archive afterward. GitHub rename redirects make any
intermediate rename cheap, but the map above is the target —
intermediate names are not worth a cascade.

## Why

- The user-visible structure kept drifting because names encoded
  history (graphopt was "the torch repo," mlopt "the product")
  rather than function. Layers encode function.
- Every dissolution step is *relabeling + moving*, not rewriting —
  the code already sits at the right seams (Source/Sink ports,
  play.register(), codec modules). The map makes the moves
  mechanical instead of exploratory.
- `meta/` tier resolves the gamekit/discoverykit/benchkit
  classification honestly: they are libraries for the
  self-improvement loop, a different axis than interpret/emit.

## Not doing

- **No big-bang migration.** Dissolution is sequenced per the
  table; each row lands behind its phase gate. Repos archive only
  after contents move AND pins repoint.
- **No `opt-*`/`engine-eqsat` renames before their phase.**
  `eqsat` keeps its name until P2 actually builds the
  interpretation; renaming ahead of the code is cosmetics.
- **No sdkgen-python now.** Deferred per above.
- **`mlopt` repo name is preserved until dissolution** — it is
  the product surface today; renaming it to kit-mlopt happens
  when the kit actually exists.

## Consequences

- New work cites the layer, not the repo history: "this is a
  compile-* emitter" / "this is kit data" / "this is meta."
- Reviewers (arch_review protocol) check repo→layer conformance
  against THIS map.
- The `myco`/sturn/pin machinery is unaffected — dissolutions are
  ordinary repo operations executed in sequence.
