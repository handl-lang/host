---
title: 'Spike: `hnl run` — the ADR-0175 pipeline-script layer'
type: report
date: 2026-11-15
---

# Spike: `hnl run` — the ADR-0175 pipeline-script layer

Date: 2026-11-2x · Status: feasibility spike (prototype evidence, not
production) · Prototype: `exec-tree-walk/tests/hnl_run_spike.rs`
(scratch test, uncommitted-to-shipped-behavior — it *is* committed as a
test file in that repo's working tree).

## TL;DR

`hnl run` is **already real** — `hnl-ext-eval` ships `hnl run
[file.hnl]` / `hnl eval '<expr>'` as an ADR-0149 cdylib extension, and
the canonical kit already parses and evaluates `a >>> b` surface syntax
as `Term::Seq` axis-0 composition. ADR-0175's "pipelines are Terms"
needs **no new eval machinery**: a stage-op is either a `Foreign` op
resolved through the kit's `ModuleLookup` chain (Rust host `Function`s),
or a `.hnl`-backed `HandlModule` function (pure-handl stage bodies). The
only genuinely new work is *registering the meta-tier stage-op modules*
(gamekit/discoverykit/benchkit → `lift`/`search`/`verify`/`mint`/`bench`)
into whatever kit assembles the pipeline script's runtime — and deciding
whether Python-backed stages wrap `python -m …` in a thin Rust
`Function` (the `hnl-ext-mlopt` precedent says yes, ~90 lines).

## 1. How `hnl` dispatches commands today (ADR-0149)

`hnl` is a **dumb host** (`handl-lang/host-cli`,
`crates/host-cli/src/lib.rs`): it discovers `libhnl_ext_<name>.{so,dylib,dll}`
via `--ext-dir` → `$HANDL_EXT_DIR` → `~/.handl/ext/` → dev
`<exe>/` dirs (`handl-ext` crate's `discover`, sibling `kernel` repo),
reads each extension's JSON manifest
(`{"name","version","commands":[{name,usage}]}`), and dispatches argv
to `handl_ext_dispatch(cmd, {"args","cwd","env"}) → {code,stdout,stderr}`.
Host-level meta commands are only `help`/`version`/`ext list`/`ext doctor`;
everything else is extension-owned. Multi-word surfaces (`compile js`)
are the extension's parsing problem — the manifest declares the first
word only.

The ABI crate is `handl-ext` (`kernel/crates/handl-ext`): three exported
symbols (`handl_ext_manifest` / `handl_ext_dispatch` / `handl_ext_free`),
JSON over a C boundary, `handl_ext::export!` macro generates the shim.

**The precedent**: `mlopt/crates/hnl-ext-mlopt` (~125 lines) declares one
namespaced command `mlopt` and forwards argv verbatim to
`python -m mlopt` — pure packaging, no engine knowledge. This is the
shape any Python-backed stage-op wrapper would take.

**Important**: `hnl run` is *already claimed* — `ext-eval`
(`handl-lang/ext-eval`, `hnl-ext-eval` cdylib) implements
`run [--engine <name>] [--name <expr>] <file.hnl>` and
`eval '<expression>'`. Its `run_file` does project-aware
`Kit::discover(.handl.toml)`, canonical-dialect typecheck, `~[N]~>`
rewrite pass (`n_dim_opt`), then `kit.runtime().eval(&ast)`. So the
ADR-0175 command surface's `hnl run program.hnl # exec engines` **exists
today**; the open question is only whether pipeline *scripts* (whose
top-level form is `lift >>> search >>> verify`) evaluate through it —
and they should, since `>>>` is surface syntax (below).

## 2. Can a `.hnl` script express a pipeline? — YES, already

`>>>` is first-class surface syntax in the canonical kit:
`kit-prelude/crates/kit-prelude/tests/fp_integration.rs:185-199` runs
`kit.run("1 >>> (x ~> x + 1) >>> (y ~> y * 2)")` → 4 (left-assoc,
`>>>` binds tighter than `~>`; `a >>> f` applies `f` to `a`).

At the Term level, `>>>` is `Term::Seq { axis: 0, fst, snd }`
(`kernel/crates/…/engine/src/term.rs:92`). The tree-walk engine
(`exec-tree-walk/src/lib.rs:913`) evaluates axis-0 Seq on
1-cells as **sequential composition**: `snd` is evaluated with `fst`'s
result as its `incoming` value. Higher axes (`>>>[k]`, k>0) are globular
pastes — out of scope for pipelines.

So `lift >>> search >>> verify` parses to nested `Seq`s and the engine
already knows how to run them — **no new eval scheme or behavior
needed**. What each stage *is* is the placement question (§3).

## 3. Where do stage-ops live?

Three viable placements, all existing seams:

**(a) `Foreign` ops → `ModuleLookup` host functions.** `Term::Op` with
`ArrSource::Foreign(name)` resolves `name` through
`TreeWalkEngine::with_modules(Rc<dyn ModuleLookup>)`
(`exec-tree-walk/src/lib.rs` ~line 2280) →
`ModuleRegistry::lookup` (`host/crates/plugin-module`) →
`Function::call(args: &[serde_json::Value], ctx)`. This is the same
chain `call_host` and `import` already use; the canonical kit wires a
`TableModuleLookup`/stdlib `ModuleRegistry` into every runtime
(`kit-prelude/src/lib.rs:216,1329,1729`). **Rust-native stage bodies**
land here — e.g. a `pipeline` module with `lift`/`search`/`verify`.

**(b) `.hnl`-backed `HandlModule`s.** Kits already load `.hnl` packages
as modules (`handl-plugin-lib-handl`, `KitResolver`, `from "pkg"
import f`); a stage written in handl itself is a `HandlModule` function.
Zero Rust.

**(c) Python-backed stages.** A thin Rust `Function` impl that spawns
`python -m <entry>` and maps exit code/stdout/stderr onto
`serde_json::Value` — exactly `hnl-ext-mlopt`'s argv-forwarding shape,
one level down (per-stage instead of per-command). Keeps `hnl run`
thin; Python stays the implementation language for the meta tier, as it
is today (`python -m bench.drivers.mint.*`, `python -m
mlopt_kernel.play`).

**Caveat found empirically**: `Foreign` ops do *not* automatically
receive the `Seq`-threaded `incoming` value — only their evaluated args
do (args are evaluated under the same `incoming` context). Threading
the pipeline record into a stage requires an arg term that yields
`incoming` — the native `id` op does exactly that
(`exec/src/native_ops.rs:682`: `id` returns `incoming`). The
prototype uses `Term::op_foreign("pipeline.lift",
[Term::op_native("id", [])])` per stage. A production `run` layer would
want a first-class "this/incoming" term or an `arr`-form stage so
scripts don't spell `id` — worth noting as the one small ergonomic gap.

## 4. Prototype evidence

`exec-tree-walk/tests/hnl_run_spike.rs` — two tests, both pass
(`cargo test --test hnl_run_spike`, 0.00s after build):

- `seq_pipeline_of_foreign_stage_ops_round_trips`: builds
  `Seq(Seq(lift, search), verify)` as `Term::op_foreign` stages over a
  `ModuleRegistry` with a `pipeline` module (three `Function`s that
  transform a JSON record), installs it as the eval scheme for AST node
  kind `"pipeline"`, and runs `TreeWalkEngine::eval` — the same entry
  `hnl run`/`hnl eval` use. Result:
  `{"stage":"verify","ok":true,"model":"demo.torch"}` — the record
  threaded lift→search→verify end-to-end.
- `seq_pipeline_failure_propagates`: an unregistered stage name fails
  loudly through `ModuleLookup` (no silent pass-through).

This is the "ONE stage-op round-trip end-to-end" the spike asked for:
script-shaped pipeline → `Term::Seq` → engine dispatch → kit-registered
stage-op → threaded value → result.

## 5. What `hnl run` minimally needs to be real

1. **Nothing for the command itself** — `hnl-ext-eval` already provides
   `hnl run file.hnl` with kit discovery, typecheck, and eval. A
   `script.hnl` containing `lift >>> search >>> verify` evaluates today
   *if the names resolve*.
2. **Stage-op registration** — the meta-tier kits need to contribute a
   `pipeline`-style module (Rust `Module`s for heavy stages, `.hnl`
   `HandlModule`s or Python-forwarding `Function`s for the rest) into
   the kit's `ModuleLookup` chain. Mechanically this is "register a
   module in `build_stdlib_registry`/kit assembly" — the kit-prelude
   composition seam already exists (`kit-*/register()` pattern,
   ADR-0128 link-time registration).
3. **Name resolution for bare stage names** — `lift`/`search`/etc. must
   be wire names (flat module-function wires already work per
   `prelude_wire_names`) or the script writes `from "pipeline" import
   lift` — both supported today.
4. **(Optional, ergonomic)** a dedicated incoming/projection term or
   `arr`-form foreign op so stages don't need the `id`-arg trick; and a
   decision on whether pipeline scripts get a distinct dialect/kind or
   are just canonical `.hnl` programs (the latter is simpler and already
   works).

**Verdict**: `hnl run` for pipeline scripts is a **thin** layer — kit
data + module registration + docs, not engine work. The heavy half
(Seq eval, Foreign dispatch, `>>>` surface syntax, `hnl run` command,
cdylib extension ABI) all exists and is tested.

## Files/commands

- Read: `host-cli/crates/host-cli/src/{main,lib}.rs`,
  `kernel/crates/handl-ext/src/{lib,discovery,load,rt}.rs`,
  `mlopt/crates/hnl-ext-mlopt/src/lib.rs`,
  `ext-eval/src/lib.rs`, `engine/src/term.rs`,
  `exec-tree-walk/src/lib.rs` (Seq ~913, Op/Foreign ~2060-2290,
  `id` op in `exec/src/native_ops.rs:682`),
  `host/crates/platform/src/eval.rs` (Function/Module/ModuleLookup),
  `host/crates/plugin-module/src/lib.rs` (ModuleRegistry),
  `kit-prelude/crates/kit-prelude/src/lib.rs` (registry wiring),
  `host/project/adrs/0175-*.md`.
- Wrote: `exec-tree-walk/tests/hnl_run_spike.rs` (scratch
  prototype, 2 passing tests).
- Ran: `cargo test --test hnl_run_spike` in `exec-tree-walk`
  (git-dep build; `[patch]` warnings are pre-existing workspace noise).
- Not done: no changes to shipped commands, no push, no `.hnl`
  surface-level pipeline script run through `hnl-ext-eval` itself
  (would need the full kit-prelude build — the Term-level proof covers
  the engine question; surface `>>>` is already covered by
  `fp_integration.rs`).
