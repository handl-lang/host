# eqsat gpt2-scale profile — where the 40+ min goes

Date: 2026-11-16. Repos: `handl-lang/engine-eqsat` @ b8debe8 (+2 newer
local commits from a concurrent agent), `handl-lang/egraph` @ 7bed824
(+ uncommitted `EQSAT_DEBUG` probe, see below).

## TL;DR

The hang is **not** term ingestion, merging, or extraction — it is a
single rewrite **guard evaluation**. `saturate_roots` stalls inside
`apply_rule(transpose_noop)` on the *check* hook (`cond =
["axes-noop","u","D0","D1"]`), specifically inside shape inference
(`tshape` → `infer_postorder`). Per-substitution check times grow
13 ms → 6.5 s and keep climbing; 52 of the 73 laws in the mlopt
search set carry `cond` guards, so the same wall is hit repeatedly.

## Evidence

Spec: `/home/luis/tmp/mlopt-verify/gpt2-spec.json` — the exact
`eqsat-run` stdin dumped for `arch:gpt2-small`
(`dump_gpt2_spec.py`): 1 `dag.program` term (669 wire nodes → 614
enodes / 614 classes), 73 `LAW_FORMAT=2` laws (52 with `cond`),
`max_iterations=1`, `max_nodes=10000`, size cost.

Stage timers (`EQSAT_TIMING=1`, engine-eqsat b8debe8 + egraph
7bed824):

```
[eqsat] compile_rules: 0.5ms (73 laws)
[eqsat] add_json: 1.3ms -> enodes=614 classes=614
[eqsat] iter 0: start enodes=614 classes=614 dirty=614 search=614
[eqsat]   rule id_add          0.1ms ...   (rules 0–12 all <0.2ms)
[eqsat]   rule select_mul      0.1ms enodes 628->628
   … then silence: apply_rule(transpose_noop) never returns.
```

Term ingestion is 1.3 ms; rule compilation 0.5 ms; the first 13
rules fire in <2 ms total. The run then burns CPU indefinitely
inside law #14 (`transpose_noop`) — matching the reported >40 min /
>2400 s `eqsat-run` timeouts in `gpt2-small-drive*.log`.

### Bisection (all runs `timeout`-bounded, `EQSAT_TIMING=1`)

| spec | laws | result |
|---|---|---|
| `gpt2-spec-14laws.json` | first 14 (through `transpose_noop`) | hangs >120 s inside `transpose_noop` |
| `spec-tn-only.json` | `transpose_noop` alone (with `cond`) | hangs >45 s, 3/3 runs |
| `spec-tn-nocond.json` | same 14 laws, `transpose_noop.cond=null` | **whole run 11.2 ms**; rule itself 8.9 ms |
| `spec-mulsq-tn.json` | `mul_square` + `transpose_noop` | hangs in `transpose_noop` |
| `spec-nomulsq.json` | 14 laws minus `mul_square` | still hangs |

So the trigger is purely the **`cond` guard**, not graph state from
earlier rules and not the matcher/union path.

### Per-substitution breakdown (temporary `EQSAT_DEBUG` probe in
`handl-egraph/src/egraph.rs::apply_rule`, env-gated, uncommitted)

```
rule=transpose_noop eid=92  subst#0  resolved 0.1ms  check 13.5ms  ok=false
rule=transpose_noop eid=129 subst#0  resolved 0.1ms  check 140.2ms ok=false
rule=transpose_noop eid=139 subst#0  resolved 0.1ms  check 201.2ms ok=false
rule=transpose_noop eid=141 subst#0  resolved 0.2ms  check 514.8ms ok=false
rule=transpose_noop eid=178 subst#0  resolved 0.2ms  check 3740.1ms ok=false
rule=transpose_noop eid=188 subst#0  resolved 0.1ms  check 2848.9ms ok=false
rule=transpose_noop eid=190 subst#0  resolved 0.1ms  check 6492.2ms ok=false
… (36 candidate classes; each `u` binds a larger/more-shared term)
```

`resolve` (e-class → min member via `min_term_cached`) is uniformly
~0.1 ms. `check` — the `axes-noop` guard — is the entire cost, and
it grows superlinearly with the resolved term's sharing.

## Mechanism

`axes-noop` calls `tshape(env, "u")` → `spec_shape_with` →
`shape_of_with` → `infer_postorder`
(`engine-eqsat/src/shape.rs:196`). The memo is

```rust
pub type Memo = HashMap<Term, TensorInfo>;   // shape.rs:158
```

`handl_egraph::Term` is an **Rc-shared DAG** (`Op.args: Rc<[Term]>`,
`term.rs:39-45`, derived `Hash`/`Eq` are structural). Every
`memo.contains_key` / `memo.get` / `memo.insert` in
`infer_postorder`/`infer_op` therefore hashes/compares the *fully
expanded* term — cost O(2^sharing-depth). The gpt2 `dag.program`
input has deep subtree sharing (the lifter hash-conses), so a single
shape lookup on a mid-graph `u` costs seconds; the memo never gets a
chance to help because each lookup itself is exponential. The same
`HashMap<Term,_>` pattern does not appear in extraction:
`egraph/src/extract.rs:46-66` already solved this exact problem with
`TermKey` — a pointer/len identity key — in commit bfe8066
("identity-keyed cost memo for shared DAG terms"). Shape inference
never got the same treatment.

Secondary effect: `min_term_cached` (`egraph.rs:1379`) *constructs*
these shared terms cheaply (per-class DP + Rc-shallow clones), which
is why `resolve` stays at 0.1 ms while `check` explodes — the bomb
is specifically structural `Term` hashing inside guard evaluation.

## Why it looked intermittent

`tn3-timing.log` (01:34, pre-reboot) shows the same spec's
`transpose_noop` completing in 36.7 ms. All shape/guard/egraph
source commits predate that run, so the likeliest explanations are
(a) the 01:34 binary was built from a working tree that had
uncommitted fixes lost in the reboot, or (b) candidate order /
`u`-binding contents differed. Post-reboot the hang reproduces
3/3 — treat it as deterministic until proven otherwise.

## Stage attribution (answer to the original question)

| stage | cost on gpt2-small spec |
|---|---|
| `compile_rules` (73 laws) | 0.5 ms |
| `add_json` term ingestion (dag.program → 614 enodes) | 1.3 ms |
| `saturate_roots` rule firing + merging | **≫40 min — guard-eval bound** |
| `extract_best_json` | never reached (unmeasured; prior micro-runs ~50-100 ms on this graph) |

## Ranked fix recommendations

1. **Key the shape `Memo` by term identity, not structure** —
   port `extract.rs`'s `TermKey` (pointer+len+op identity) or
   intern/hash-cons guard terms. Turns O(expanded-tree) hash+eq
   into O(DAG-node). Smallest change, fixes every `cond`/`dspec`
   shape guard at once. Watch: `TermKey` is pointer-based, so it is
   only valid within one inference call — which is exactly the
   `Memo` lifetime.
2. **Cache shapes per e-class** — guards resolve `u` to a class
   member anyway; a `shape_of_cached(eid)` analogue of
   `min_term_cached` would dedupe across the 36 substs *and* across
   laws/rules/iterations, not just within one `tshape` call.
3. **Give `check`/`derive` hooks a work budget** (node-visit cap →
   decline). A guard that cannot afford its answer should veto the
   rewrite, never hang the search — cheap insurance for the next
   pathological term.
4. **Structural `Hash`/`Eq` on `Term` is a footgun everywhere** —
   audit other `HashMap<Term,_>`/`BTreeMap<Term,_>` uses
   (`shape.rs:158`, `egraph.rs:265 progs`, `extract.rs:367 seen`)
   for the same exposure; consider a cached-hash wrapper or
   hash-consed term IDs long-term.

## Housekeeping notes for the parent

- **Do not push yet.** `engine-eqsat` HEAD has moved: two commits
  from a *concurrent agent* (`c886f9f`, `fe6353d` — prior_art law
  ports) now sit on top of b8debe8, plus uncommitted
  `src/builtin.rs`/`tests/contracts.rs` work-in-progress. b8debe8
  itself is clean and env-gated (`EQSAT_TIMING` only; zero output
  when unset), so it is push-safe *content-wise* — but pushing
  `b8debe8:main` mid-flight under another agent's commits is a
  coordination call. Its pair commit `handl-lang/egraph@7bed824`
  (per-rule/phase counters) is also unpushed; engine-eqsat's stage
  timers still work without it (just no per-rule lines). Recommend
  pushing both together once the law-port agent lands.
- **Uncommitted probe left in `handl-lang/egraph/src/egraph.rs`**:
  `EQSAT_DEBUG`-gated per-subst resolve/check timers in
  `apply_rule` (~30 lines, zero cost when unset). Keep as
  diagnostic or `git checkout` to revert — parent's call.
- `cargo fmt --check` fails on *committed* code
  (`tests/eqsat.rs` since 0397980) and rustfmt wants import
  reordering in b8debe8's files — local rustfmt style drift, not a
  b8debe8 regression.
- `/tmp` (tmpfs) hit quota mid-session causing phantom exit-101s;
  all run artifacts are under `/home/luis/tmp/mlopt-verify/`
  (`gpt2-prof-timing.log`, `t-tn-dbg.log`, `spec-*.json`, `o-*.json`).
