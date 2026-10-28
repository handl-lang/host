# Multi-shot `resume` on the cranelift engine — landing note

## Status

Landed. `resume` (ADR-0158 replay-journal model) runs on all three eval
engines: tree-walk (reference), bytecode (native replay) and cranelift
(journal through the shared `handle_dyn` hook + the tree-walk arrow
interpreter). Design: `design/multishot-resume.md`.

This note is the changeset manifest — the working tree is not a git
checkout, so the file inventory below is the hand-off record.

## Summary

A handler clause may take a second parameter — the resumption:

```hnl
handle perform(choose, [1, 2]) + 100 with {
  choose: (opts, resume) ~> [resume(opts[0]), resume(opts[1])]
}
-- → [101, 102]
```

`resume(x)` re-runs the handled body from the `handle` boundary under a
replay journal seeded with `entries[..i] ++ [(op, x)]`; the body's
completed value (through the `return` clause) is `resume`'s result. No
continuation capture: the journal is data, so `resume` is copyable.

### Cranelift specifics

- `NativeOpCtx::run_delimited` override installs the journal when the
  frame has no `__acc__` slot and the handler has a ≥2-ary clause, drives
  the body, then runs the `IO`-former + carried-handler postlude.
- `perform` is intercepted at every dispatch seam (`call`, `flow`,
  `call_host`, and `arr <op>`) before one-shot `perform_dispatch`.
- `__resume__` values are intercepted in `apply_closure_value_opts`,
  including the single-`Arr` unpack a `call` performs on positional args.
- Cranelift arrow values run in the tree-walk interpreter
  (`apply_value_treewalk`), so a `resume` inside a clause body is applied
  by the *tree-walk*. `native_ops` hosts an engine-neutral driver table
  (`register_resumption_driver` / `drive_shared_resumption`) that bridges
  the two; `ACTIVE_EVAL_CTX` (installed for the duration of a clause
  application) recovers `&mut EvalCtx` on the cranelift side.
- Compiled-codegen unwinding: `helper_v2_error_is_suspended` makes
  `compile_args_v2` and `Seq` stop evaluating after a suspension, so the
  journal cursor stays at the suspending op. `EvalCtx::set_error` never
  overwrites a `Suspended`, and the journal latches its first suspension
  as a second guard.
- Lifecycle: `native_ops::ResumeTableScope` clears the shared driver
  table plus each engine's journal/suspension tables when the outermost
  eval on the thread returns (nested evals only bump a depth counter).

## Files changed

| File | Change |
|---|---|
| `engine-exec/src/native_ops.rs` | `ResumptionDriver` + shared driver table, `ResumeTableScope` (+ eval depth), rec-region fixpoint propagates `Suspended` instead of swallowing it, style cleanups |
| `engine-exec/src/{io,lib,pattern_match}.rs` | clippy cleanups (no behaviour change) |
| `engine-exec-cranelift/src/lib.rs` | journal driver (`ResumeJournal`/`ResumeSuspension`/`drive_journal`/`resume_from`/`resume_suspension`), `run_delimited` override, perform/`arr` seams, `__resume__` seam, `ACTIVE_EVAL_CTX`, `set_error` guard, `helper_v2_error_is_suspended`, table scope, node-accurate diagnostics, 5 tests |
| `engine-exec-cranelift/src/compile.rs` | declare/register the new helper, suspension short-circuits in `compile_args_v2` and `Seq{axis:0}`, item allows |
| `engine-exec-cranelift/src/standalone.rs` | treat a staticlib older than the crate's newest source as missing (rebuild) — a new `helper_v2_*` otherwise fails at `cc` link time |
| `engine-exec-cranelift/tests/standalone_bootstrap.rs` | skip the `multishot_resume` corpus case (standalone has no `Language` for arrow application) |
| `engine-exec-cranelift/tests/{object_compilation.rs,conformance/harness.rs}` | clippy cleanups |
| `engine-exec-tree-walk/src/lib.rs` | `__resume__` seam consults the shared driver table; table scope in `eval_ast_rc_tagged`; cleanups |
| `engine-exec-bytecode/src/lib.rs` | table scope in `eval_ast_rc_with_args` |
| `engine-exec-bytecode/src/vm.rs` | 3 resume tests (both continuations, nested cartesian, one-shot regression) |
| `engine-exec-bytecode/src/compile.rs` | clippy cleanups |
| `host/crates/handl-test-utils/src/conformance/corpus.rs` | `multishot_resume` eval-corpus case — the cross-engine gate |
| `host/crates/handl-runtime/tests/engine_parity.rs` | `perform_multishot_resume` (both continuations / nested / escape, all three engines); fixed the `vsum<n>` self-recursion case (`{n}` → `<n>`, the pre-existing failure was a test-source syntax bug) and extended it to all three engines; added `self_recursive_arrow_without_data_types` (fac(4) = 24 on every engine), which shows self-recursion is *not* an engine divergence |
| `host/project/adrs/0158-*.md`, `host/project/design/multishot-resume.md` | status, deltas, limits |
| `engine-parse-pratt/src/{lib,former_decl}.rs`, `host/crates/plugin-ext-typecheck/tests/scratch_ast_dump.rs` | incidental clippy fixes (crate test suites re-run green) |

## Verification

```
cargo test                 # engine-exec           13 passed
cargo test                 # engine-exec-cranelift 104 passed
cargo test                 # engine-exec-bytecode   94 passed
cargo test                 # engine-exec-tree-walk 142 passed
cargo test --test engine_parity   # host/crates/handl-runtime  44 passed
cargo clippy --all-targets # engine-exec / -cranelift / -tree-walk / -bytecode
                           # → no warnings (was ~250 across the four crates)
```

The clippy pass was mechanical (machine-applicable fixes + `cargo fmt` on the
touched files) plus item-level `#[allow]`s for design lints
(`too_many_arguments`, `large_enum_variant`, `result_large_err`,
`type_complexity`) and `# Safety` sections on the cranelift `helper_v2_*`
ABI functions.

### Pre-existing failures (not from this changeset)

`engine-parse-pratt` is **being edited concurrently** — its `HEAD` advanced
mid-session (`3fb1a4e feat(pratt): lower every family-record decl, not only
the chunk root`, committed 19\:16 local) as part of an in-progress
`{…}`→`<…>` / family-record lowering workstream. While that lands, the
`data`-declaration desugar is transiently broken, so these fail for reasons
unrelated to `resume`:

- `handl-runtime` `engine_parity`: `ctor_object_match_dispatches_on_sym`,
  `nat_tower_binary_add`, `perform_acc_state_threading`,
  `self_recursive_arrow_evals_via_ast_fallback` — all tree-walk paths that
  report `call_host: no module registry` at a constructor call
  (`Jx(9)`, `Succ(Zero)`), i.e. the `data` desugar did not bind the ctor.
- `indexed_family_data` / `indexed_family_fin` / `indexed_family_match`:
  `handl:desugar.data` errors with `unexpected token: token.keyword.acc`.

**Status update:** the record→`data` lowering now exists as a runtime
pre-pass — `handl_runtime::desugar_pass::lower_family_record_decls`
(`host d7287de8`) rewrites `alt[assign(type, c0), c1, …]` in place into the
internal `data { children: [type, ctors] }` shape (and drops the retired
`data` keyword token), before `run_pass(…, "desugar")`. That fixed
`ctor_object_match_dispatches_on_sym` and `nat_tower_binary_add`:
`engine_parity` is **43/44**. It is additive — a parser that emits a `data`
node already makes it a no-op — so it can converge with (or be replaced by)
the in-flight `engine-parse-pratt` work. It only lowers *multi-constructor*
records; a one-constructor family still needs the parser-side path.

Remaining reds and what the probes show:

- `self_recursive_arrow_evals_via_ast_fallback` — bytecode returns `0`
  where tree-walk/cranelift return `2`.

  **Minimal repro (no recursion, no `toNum`)**: with
  `data Nat :: Zero | Succ(pred : Nat)` and
  `data Vec(n : Nat, a : Type) :: VNil : Vec<0, a> | VCons(x : a, xs :
  Vec<n, a>) : Vec<n + 1, a>`,
  `w : Vec ~> Nat = v ~> match v { VNil: Zero, VCons(x, xs): Succ(Zero) }`
  then `w(VCons(Zero, VNil))` gives `Succ(Zero)` on tree-walk and
  cranelift but **`Zero`** on bytecode — it selected the `VNil` arm. The
  *same* match at root position
  (`match VCons(Zero, VNil) { VNil: 0, VCons(x, xs): 1 }`) returns `1` on
  all three, so the defect is specific to matching an **indexed-family**
  ctor from inside an arrow body, not to ctors, binders, or recursion.

  Narrowed further: the signature is irrelevant —
  `w : Number ~> Number = p ~> match p { VNil: 0, VCons(a, b): 1 }`
  applied to `VCons(Zero, VNil)` also returns `0` on bytecode (`1` on
  tree-walk), while the same body returning a constant works on both, and
  a **non-indexed** family (`data P2 :: Mk(a : Nat, b : Nat) | PNil`)
  matched on a parameter works on both. So the trigger is: an
  **index-parameterized family ctor bound to an arrow parameter** matched
  inside an arrow body.

  **Mechanism (instrumented):** at the match, the arrow's captured map is
  just `["VCons"]` — the nullary ctor's name is missing. So
  `PatternMatchCtx::scope_value("VNil")` returns `None`, `pattern_match`
  degrades the pattern to a *catch-all binder*, and the first arm fires
  for every value. The capture set is filtered by
  `subtree_name_candidates`; whatever mints this arrow drops the
  pattern-position name (the ctor binding itself is identical on all
  engines, and the closure *does* capture `VNil` when the body merely
  mentions it). Next step: instrument the arrow mint (which `closure`
  path runs and what the candidate set holds) rather than the match.
  Two candidate fixes were tried and reverted: a `capture = None`
  bail-out in bytecode's `Instr::Closure` fast path, and the same guard in
  the shared `closure` op — neither moved the result, so the minting path
  is a third one still to be located.

  Ruled out by probe (all three engines agree on each): `Zero == Zero`,
  `Zero == Succ(Zero)`, `Succ(Zero) == Succ(Zero)`, and
  `Zero.__ctor__ == Succ(Zero).__ctor__`; ctor arm binders
  (`match VCons(Zero, VNil) { VNil: 0, VCons(x, xs): [x, xs] }`); and
  **two-argument** ctor patterns both at root and inside an arrow body
  (`data P2 :: Mk(a : Nat, b : Nat) | PNil`). So arm *selection* is sound;
what differs is only the indexed-family case inside an arrow body. Not the implicit `<n>` parameter
  (identical with and without it), not ctor construction (all three build
  the same `VCons` object), not arm binders (all three bind `x`/`xs`
  identically), not plain ctor match (`match VCons(Zero, VNil) { … }` = 1
  everywhere), and not recursion in an arm (`toNum` over `Succ(Succ(Zero))`
  = 2 everywhere). The divergence is confined to a recursive arrow whose
  *result* is itself a constructor (`Succ(vsum(xs))`) applied at the top
  level — the returned Nat-tower terminator suggests `vsum` yields a
  non-Nat value there and `toNum` falls to its base arm.
- `perform_acc_state_threading` — tree-walk returns `Null` instead of `1`
  for the ADR-0158 `acc` clause; unrelated to `resume` and to the lowering
  (it failed identically before this pass).

**Fixed (cranelift, `engine-exec-cranelift`):** the compiled `{__term__,
__bound__}` arm appended the *packed* call args to `__bound__`, so
`Type.rec(m, c0..)(v)` built `[motive, c0.., [v]]` and the fold asked an
array for `.__ctor__`. It now unpacks a single `Arr` argument (guarded by
`unpack_arr`, so `flow`/direct `__term__` callers are untouched) — the
match on `data Nat` works on cranelift again.

Note also that `lower_family_record_decls` only lowers **multi-constructor**
records, so a one-constructor family (`data P2 :: Mk(a : Nat, b : Nat)`
with no second ctor) is still unbound; that needs the parser-side path.

Diagnosis for whoever lands that work. Dumping the AST for
`do { data Opt2(a : Type) :: Nope | Jx(x : a)  match Jx(9) { … } }` shows
the parser now emits `var "data"` as a stray statement plus a family-record
`alt` of `assign`s — `Opt2(a : Type) := Nope | Jx(x : a)` — and nothing
lowers that record into a `data` node with former/ctor `assign`s, so `Jx`
stays an unbound name and `Jx(9)` falls through to `call_host`. The same
shape appears with the post-ADR-0158 spelling (no `data` keyword), so the
missing piece is the record→`data` lowering, not the desugar rule: the
`.hnl` rule (`lib-desugar/src/index.hnl::data_rewrite`) only ever sees a
`data` node, which is what the lowering is supposed to produce. The `acc` in the other error is the ADR-0158 `acc` keyword
colliding with identifiers in a desugar rule source — `lib-desugar/src/
index.hnl` uses `acc` as a record field / binder (89 lines), which the
lexer now classifies as `token.keyword.acc`; renaming those binders (or
teaching the grammar to accept the keyword there) is a prerequisite. That
file has no tests, so the rename was left out of this changeset.

Every `perform`/`resume` case in the parity suite is green, including
`perform_multishot_resume`; `engine-parse-pratt`'s own 62 unit tests pass.

Also fixed here (was a genuine test bug, not an engine bug):
`engine_parity`'s self-recursion source spelled implicit parameters
`vsum{n}` / `Vec{n, …}`, but the language uses `<n>` — the parser silently
produced a degenerate AST, so `vsum` was never bound. Corrected to `<n>`;
tree-walk and cranelift yield `2`, bytecode's `Zero` is pinned as a known
divergence.

`host/crates/plugin-ext-typecheck/tests/` (untracked) was touched by a stray
`cargo clippy --fix` run (formatting only) and re-checks green.


## Known limits

See `design/multishot-resume.md` §"Known limits": deterministic bodies
only; `resume` escapes are supported while the eval is live, not across
evals (the tables are reclaimed at the eval boundary); `perform` inside a
`rec`/loop fixpoint region now propagates instead of being dropped but is
still not a supported position.

Self-recursion itself is engine-independent: a `letrec` arrow
(`fac := (n) ~> … fac(n - 1)`), with no `data` declaration involved,
returns `24` on tree-walk, bytecode *and* cranelift. The `vsum → Zero`
result seen earlier was the broken `data`/ctor lowering (unresolved
patterns fall through to the base arm), not a bytecode bug.

## Landing

Landed per repository (the workspace is a set of sibling git repos; the
root is not itself a checkout). Exact paths were staged — never `-A`,
since several repos carry unrelated in-flight work.

| repo | commit |
|---|---|
| `engine-exec` | `3e9d705` native_ops: shared resume-driver table + eval-scope cleanup; clippy clean |
| `engine-exec-cranelift` | `34aa147` cranelift: multi-shot resume (ADR-0158 replay journals) + suspension-aware codegen |
| `engine-exec-tree-walk` | `18a006d` tree-walk: consult the shared resume-driver table; eval-scope cleanup |
| `engine-exec-bytecode` | `202ffea` bytecode: multi-shot resume tests + eval-scope cleanup; clippy clean |
| `host` | `c4d85a25` tests: multishot resume parity + eval-corpus case |
| `host/project` | this note + ADR-0158 + the design doc |
| `automation` | `scripts/check-engines.sh` (fmt + `clippy -D warnings` + tests for the four engine crates) |

Run the gate with:

```
./automation/scripts/check-engines.sh          # fmt + clippy + tests
./automation/scripts/check-engines.sh --quick  # fmt + clippy only
```
