---
title: Audit pass 18 — ADR-0179 __tensor_bin__ wire audits clean across kernel/host-tensor/graphopt (eq/hash, threshold, base64, recursion, payload-as-wire all verified); hnl-run egress covered but ext-eval `hnl run` print path still leaks bin records (named follow-up); engine-eqsat arr-leaf + parse-pratt multi-statement + whisper alias meta all clean; kit-discovery still unlanded
type: report
date: 2026-11-17
---

# Audit pass 18

Scope: commits since pass 17 — kernel `9d9e875` (handl-value
`TensorData::{F64,F32}` + `__tensor_bin__` wire, ADR-0179),
host-tensor `3af18c0` (codec bin arms + `tensor.load` payload-as-wire)
+ `c488bad` (ranged safetensors streaming, committed after pass 17's
skim), graphopt `069d7db` (hnl-run materialize + `wire.py` decode
arm), engine-eqsat `dd4815e` (`{"Arr": …}` leaf records),
tool-parse-pratt `50070dc` (multi-statement parse), mlopt `bd386b7`
(whisper decoder alias meta), and the D-1d kit-discovery state.

**WIP present and not disturbed:** kernel has two uncommitted files
(`Cargo.lock` +11 — `handl-kit-mlopt` lock entries from a kit-prelude
build; `crates/handl-value/src/nbe.rs` — verified `git diff -w` is
**pure rustfmt reflow**, no semantic change); graphopt carries the
same ~uncommitted D-1d cascade as pass 17; mlopt carries its 9-file
cascade plus untracked `arch-gpt2-small.hnl`; kit-discovery still has
237 staged-but-uncommitted files.

## Per-commit verdicts

### kernel `9d9e875` + host-tensor `3af18c0` + graphopt `069d7db` — ADR-0179 — CLEAN; all seven audit questions verified

**(a) PartialEq/Hash through f64 widening — sound, no collision or
miss.** `TensorData::eq` (`kernel/crates/handl-value/src/lib.rs`,
`impl PartialEq for TensorData`) compares `len()` then
`iter_f64().zip().all(==)`; `f32→f64` is exact and injective, so an
F32 tensor equals *exactly* the F64 tensor holding its widened values
— the intended pre-ADR semantics, and the doc comment says so. The
converse cannot produce a false positive: an F64 tensor holding a
non-f32-representable value (e.g. `0.1f64`) never equals an F32
tensor (`0.1f32` widens to `0.10000000149…` ≠ `0.1`). `Hash`
(`lib.rs` `TaggedValue::Hash` Tensor arm) hashes `iter_f64()` with
the same `-0.0→+0.0` normalization — equal tensors hash identically.
NaN: `==` is false (tensor unequal to itself, as before); hash bits
are unconstrained for NaN, and `f32 NaN as f64` is deterministic so
equal-NaN-payload tensors at least hash stably. `TensorValue`'s
derived `PartialEq` delegates to `TensorData::eq` through `Rc` —
dtype is deliberately *not* part of equality (ADR decision,
consistent).

**(b) `TENSOR_BIN_THRESHOLD = 4096` — consistent, no off-by-one.** All
three emit sites gate on **element count** (`numel == data.len()`,
enforced by `TensorValue::new_data`'s `product(shape)` check), never
bytes: kernel `tagged_to_value` (`t.data.len() >=
TENSOR_BIN_THRESHOLD`), host-tensor `encode_tensor` (`t.data.len() >=`),
`tensor.load` (`numel >=`). All three use `>=`, matching the ADR's
"numel >= threshold" wording and mlopt's `_SIDECAR_MIN_ELEMS`
convention. `tensor_bin_record`/`tensor_bin_decode` re-verify
`payload.len() == numel × itemsize` with `checked_mul` on both sides.

**(c) Vendored base64 — correct.** `b64_encode`/`b64_decode`
(`lib.rs` ~line 1860+): standard RFC-4648 alphabet with `=` padding,
output always multiple of 4. Decode is strict on length (`% 4`),
padding position (`=` only trailing, ≤2, none in the prefix), and
alphabet membership. INFO nit: it does not check pad *bits* are zero
(`"AR=="` decodes same as `"AQ=="`) — lenient but produces identical
bytes; harmless.

**(d) `materialize_tensor_bin` — recursion safe.** `serde_json::Value`
is a tree (no cycles possible); objects recurse into all values,
arrays into all elements, so tensors nested in `binds`/records
materialize. The `{"__tensor__": …}` inner is visited twice (once via
`materialize_tensor_bin_inner`, once via the `values_mut` loop) —
second pass is a no-op since `data` is already an array.
`materialize_tensor_bin_inner` widens via `tensor_bin_decode` →
`to_f64_vec` → `encode_f64_element` (`__nonfinite__` tags restored)
and restamps `dtype: "f64"`; malformed records are left untouched.
Cycle-safe by construction; depth-bounded only by JSON nesting.

**(e) `tensor.load` payload-as-wire — dtype/endianness/nonfinite all
preserved.** `load_safetensors` (`host-tensor/src/ops/codec.rs:578`)
returns `serde_json::Value` now; for `numel >= 4096` it seeks to
`hend + chosen.begin`, reads `nbytes` once, and emits
`tensor_bin_record(shape, "f32"|"f64", payload)` straight from the
safetensors bytes — which *are* LE IEEE by spec, matching the wire's
LE spelling. `entry_decode_plan` (`codec.rs:852`) validates dtype
(`F64`/`F32` only — F16/BF16/I64 stay named `EvalError`s *before* any
payload read) and `nbytes == numel × itemsize` is checked before the
seek. Ordering is safe: the `data_offsets` bounds check
(`chosen.end > payload_len || chosen.begin > chosen.end`,
`codec.rs:613`) runs **before** `chosen.end - chosen.begin`
subtraction — no underflow. Raw non-finite bits ride verbatim (no
decode at all on this path).

**(f) `wire.py` decode arm — correct.** `_decode_bin_data`
(`graphopt/packages/mlopt-host-tensor/src/mlopt_host_tensor/wire.py:131`):
dtype must be `"f32"`/`"f64"` (required — missing/other raises
`HostTensorError`), `base64.b64decode(validate=True)`, payload length
must equal `numel × itemsize`, `np.frombuffer(dtype="<f4"/"<f8")`
(explicit little-endian) `.astype(np.float64)` — f32→f64 exact, NaN
preserved as NaN. `from_wire` only takes the bin arm when `data` is a
dict containing the tag; anything else falls to the array path which
raises on non-numbers. `to_wire` unchanged (array only) — symmetric
with the Rust side.

**(g) Egress coverage — hnl-run covered; `hnl run` gap is the named
follow-up, still open (P18-N1).** `hnl-run.rs:110-116` calls
`handl_value::materialize_tensor_bin(&mut v)` before `println!`, so
the mlopt-kit stdout contract (`wire["data"]` is a list) holds.
`egraph-run` prints e-graph reports — kernel `Term`s carry no tensor
payloads, not an egress. **But** `ext-eval`'s `print_value`
(`ext-eval/src/lib.rs:595`, call sites `:213`/`:259`) prints the eval
`Value` with no materialize — `hnl run`/`hnl eval` on a program whose
result contains a ≥4096-element tensor emits the base64 record
unmaterialized. ADR-0179 explicitly tracks this as a follow-up
("`hnl`/`hnl run` and other `Value` egress points must call
`materialize_tensor_bin` before printing — tracked as a follow-up,
not silently left emitting base64"), so it is a *documented* gap, not
a silent one — but it is still open and is the only known uncovered
stdout boundary. Other `tagged_to_value` sites
(`host/crates/handl-runtime/src/vm_primitives.rs:504`,
`plugin-lib-handl` module serialization) are internal
serialize/deserialize paths that round-trip through
`tensor_from_json`, which handles both spellings — not leaks.

Also verified: `tensor_from_json` (kernel decode) requires `dtype`
for the bin spelling and falls back leniently to plain object on any
malformation; host `decode_tensor` is strict `TypeMismatch` — the
documented asymmetry. `object_view`'s Tensor arm still emits
`dtype:"f64"`+array for F32 storage — self-consistent (values are the
widened f64s), noted only. Tests: kernel `cargo test -p handl-value`
→ 21/21 tensor tests pass (incl. `tensor_eq_and_hash_across_dtypes`,
`tensor_encode_bin_preserves_f32_dtype`,
`materialize_tensor_bin_expands_to_array_form`,
`tensor_bin_is_portable`); host-tensor `cargo test --lib ops::codec`
→ 35/35 pass; graphopt `pytest tests/test_host_tensor_wire.py` → 5/5
pass (but see P18-N2 — no bin-arm coverage there).

### host-tensor `c488bad` — streaming `tensor.load` — CLEAN (pass-17 skim promoted to full audit)

- Header length: `hlen` is `usize::try_from`'d, then `8 + hlen <=
  file_len` is checked via `checked_add`+`filter` **before**
  `vec![0u8; hlen]` allocates (`codec.rs:596-604`) — a corrupt prefix
  can't ask for more than the file holds. INFO: a *valid-length* huge
  header still allocates O(hlen); safetensors headers are small and
  this matches the pre-existing class — not a regression.
- `payload_len = file_len - hend` can't underflow (`hend <= file_len`
  enforced); `data_offsets` bounds-checked before use.
- `decode_entry_streaming` (`codec.rs:~810`): `CHUNK = 1 MiB` is a
  multiple of both item sizes (8 and 4); `remaining` starts at
  `expected` (a multiple of `elem_size`) and `take = min(remaining,
  CHUNK)` stays a multiple of `elem_size` — the `chunks_exact`
  decode sees only whole elements. `read_exact` into `buf[..take]`,
  `buf` is CHUNK-sized so `take <= CHUNK` always. Bit-identical to
  the old whole-buffer path: same `entry_decode_plan` decoder fns,
  same `f32→f64 as`-cast widening; the old path survives as
  `#[cfg(test)] decode_entry`/`parse_safetensors`.
- `parse_header`/`choose_entry` extraction is behavior-preserving
  (same error strings, same `__metadata__` skip, same
  unnamed-multi-tensor refusal).

### engine-eqsat `dd4815e` — `{"Arr": …}` leaf records — CLEAN

- `arr_leaf_record` (`src/dag.rs:468`) fires only on `op == "arr"`
  with exactly 1 child; then requires exactly one attr `("value",
  Lit)` — anything else is a **named refusal** (`BadTerm`), matching
  the commit's honesty claim. Verified against
  `egraph/src/term_bridge.rs`: `to_kernel`'s `"arr" if args.len()==1`
  arm (`term_bridge.rs:193`) reads *any* one-child `arr` enode as
  `KTerm::Arr` with `value` defaulting to `Null` — so a non-canonical
  `arr` enode emitted as `Op` would indeed misreport; refusal is the
  right call. A legit user op literally named `"arr"` with 1 child +
  a `"value"` attr is indistinguishable from `Term::Arr` to
  `to_kernel` anyway — emitting `Arr` is the faithful spelling.
- Multi-child `arr` enodes fall through to `op_record` (ordinary
  ops) — correct, `to_kernel` wouldn't read them as `Arr` either.
- `dim` as inline **tree** record inside the dag table is consistent:
  `add_node` decodes leaf records via
  `serde_json::from_value::<hnl_engine::Term>` (`dag.rs:154-159`),
  which parses the full nested kernel-Term JSON — no table index
  needed, matching the `__mlopt_leaf__` convention. `tree_json` on the
  dim child expands shared subtrees inline (loses DAG sharing for
  that one leaf — size cost only, semantics exact); a metavariable
  dim refuses by name like every other extraction path.
- Tests: `cargo test dag` → 6/6 pass incl.
  `dag_program_arr_leaf_survives_dag_output`,
  `dag_output_noncanonical_arr_enode_refuses_by_name`,
  `dag_program_arr_leaves_round_trip_tree_wire`.

### tool-parse-pratt `50070dc` — multi-statement parse — CLEAN

- `parse()` now collects every top-level expression into a `block`
  node (`{"children": [stmt roots]}`) when >1 — byte-shape identical
  to what `do { … }` emits, and `eval_block`
  (`exec/src/native_ops.rs:947`) evaluates children in order
  in a fresh scope, last value wins — exactly the documented
  semantics. Single-statement programs are unwrapped (root = the
  expression), so scalar/expr callers are unaffected.
- Separator tolerance: `,`/`;` are skipped by **surface label**
  (`kind_for_label(lang, ",", "token.comma")` etc.), same
  resolution `parse_delimited_block` uses — a differently-spelled
  language still parses. INFO: a language that legitimately used `,`
  or `;` as a *prefix* operator at top level would see it swallowed —
  same exposure blocks already have; theoretical.
- Trailing tokens: after the statement loop, any leftover peekable
  token that can't start an expression (`can_start_expression` =
  prefix-rule lookup, `lib.rs:~381`) is a named `LangError::Engine`
  "unexpected token after top-level expression" — `)`, `]`, `}` etc.
  can no longer be silently dropped. Tokens that *can* start an
  expression become the next statement (the `1 x` test update
  documents this: previously `x` was silently dropped, now it's a
  second statement — the honest reading).
- Old-behavior consumers: `parse()` is the only entry (no
  expression-only mode; `ParserRequest` options are unconsumed in
  this engine). The truncation was the bug being fixed — the two
  updated tests (`next_token_without_infix_rule_stops`,
  `optional_child_does_not_swallow_following_expression`) pin the new
  block-wrapped shape. No committed caller could have depended on
  truncation without depending on the bug.
- Trivia/newline tokens are ignorable via `is_ignorable` (cursor
  skips transparently), which is what lets `1+1\n2+2` split —
  verified by the passing tests.
- Tests: `cargo test` → 65 unit + 1 conformance, all pass.

### mlopt `bd386b7` — whisper decoder alias meta — CLEAN

- Position math verified against `_build_static_inputs`
  (`whisper.py:594-607`): returns `(hs, mask, pos, *(sk + sv + ck +
  cv))` with each list length `n_layers` — so positions `3..3+n` =
  sk, `3+n..3+2n` = sv, `3+2n..3+3n` = ck, `3+3n..3+4n` = cv.
  `_decoder_alias_positions` (`:610-634`) emits
  `alias = range(3, 3+4n)`, `mutable = range(3, 3+2n)` — mutable is
  exactly sk+sv (the `_KVWriteLayer` `index_copy_` write-back pair,
  `whisper.py:270-271`), ck/cv (`_KVRefLayer`, adopted read-only)
  stay alias-only. `mutable ⊂ alias` holds by construction.
- Forward's `*past` slicing (`whisper.py:357-361`) confirms the same
  order: `sk = past[0:n]`, `sv = past[n:2n]`, `ck = past[2n:3n]`,
  `cv = past[3n:4n]` — positions line up with the inputs tuple.
- Positions 0..2 (hs/mask/pos) excluded — rebuilt per step, correct.
- Encoder arm does **not** declare the keys — `_load_encoder`'s meta
  has no `alias_inputs`/`mutable_inputs` (grep-verified; only the
  decoder meta dict at `:901-902` sets them).
- Wiring verified: `ModelSpec(meta=meta)` → `optimize.py:290,315`
  passes `spec.meta` to `select_runner`/`deliver_game` →
  `_alias_spec` (`deliver.py:125`) validates to `frozenset[int]` →
  `CudaGraphRunner(alias_inputs=, mutable_inputs=)`. Declared even on
  CPU — harmless, `_alias_spec` is only consulted under
  `_has_cuda` (`deliver.py:205`).
- Tests: `pytest tests/test_whisper_adapter.py` → 15/15 pass (incl.
  the new `test_decoder_alias_positions_layout` pinning the layout,
  mutable⊂alias, and the `_alias_spec` round-trip).

### D-1d kit-discovery — unchanged; P17-N2 stands (MEDIUM)

- `git ls-remote origin` → rc=0, **zero refs** (repo still empty).
- No tags; HEAD `1d6429f`; **237 files staged but uncommitted**
  (111 `A` + 123 `R` + 2 `D` + 1 `M`, ~187k insertions).
- Consumer pins unchanged: mlopt `pyproject.toml:64,69`-area and
  kernelopt `pyproject.toml:52,57` still pin `mlopt-torch`/
  `mlopt-host-tensor`/`kit-discovery` at `v0.1.16`/`v0.1.0` — the
  uncommitted D-1d cascade diff shows the tag lines *unchanged*.
- P17-N1 also stands and is now sharper: graphopt HEAD is
  `v0.1.17 + 1` (`069d7db`, **no v0.1.18 tag exists**), so even the
  pin bump can't yet deliver the ADR-0179 graphopt-side changes —
  they need a tag first. mlopt/kernelopt remain on `v0.1.16`, two
  releases behind HEAD.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P18-N1 | LOW | `ext-eval/src/lib.rs:595` (`print_value`), call sites `:213`/`:259` | `hnl run`/`hnl eval` print eval `Value`s without `materialize_tensor_bin` — a ≥4096-element tensor result emits `{"__tensor_bin__": "<base64>"}` where consumers expect `data` as a list. This is the ADR-0179-named follow-up, still open; hnl-run (mlopt-kit) is covered, ext-eval is the remaining known stdout boundary. Loud-but-wrong-shape, not silent corruption — any JSON consumer sees a valid self-describing record. |
| P18-N2 | INFO | `graphopt/tests/test_host_tensor_wire.py` | No test exercises `wire.py`'s `_decode_bin_data` — the Rust codec arms are well-tested (35 host-tensor + 21 kernel tests) but the Python decode arm shipped untested. A bad-dtype/bad-base64/bad-length case would be cheap to pin. |
| P18-N3 | INFO | `kernel/crates/handl-value/src/lib.rs` `tensor_bin_record`/`b64_decode` | numel-0 asymmetry: `tensor_bin_record` accepts an empty payload (emits `""`) but `b64_decode("")` returns `None` — the record can't round-trip. Unreachable via any threshold-gated emitter (all require ≥4096); direct-API misuse only. |
| P18-N4 | INFO | `kernel/crates/handl-value/src/lib.rs` `b64_decode` | Non-canonical pad bits accepted (`"AR=="` ≡ `"AQ=="`); lenient but byte-identical output. Python `validate=True` behaves the same — consistent across the boundary. |
| P18-N5 | INFO | `kernel/crates/handl-value/src/lib.rs` `object_view` Tensor arm | F32-storage tensors still project `dtype:"f64"` + widened array — self-consistent (the emitted data *is* f64) but means `object_view` never exposes the narrow dtype; only the `tagged_to_value` wire path does. Documented behavior, noted for completeness. |
| P18-N6 | INFO | kernel worktree | Uncommitted `nbe.rs` is pure rustfmt reflow (verified `git diff -w` — no semantic change); `Cargo.lock` +11 is `handl-kit-mlopt` entries from a kit-prelude build. Unrelated in-flight work, not part of `9d9e875`. |
| P18-N7 | MEDIUM (stands) | mlopt/kernelopt `pyproject.toml` pins | P17-N1 unchanged and now blocking ADR-0179 delivery too: consumers pin `v0.1.16`; graphopt HEAD (`069d7db`) is untagged — needs `v0.1.18` before any pin can pick up the hnl-run/wire.py halves. |
| P18-N8 | MEDIUM (stands) | kit-discovery repo + consumers | P17-N2 unchanged: remote empty, no `v0.1.0` tag, 237 staged files uncommitted, HEAD `1d6429f`; `uv sync` still broken workspace-wide. |

No HIGH findings. The ADR-0179 implementation is careful work: the
f64-widening eq/hash is provably consistent (f32→f64 injective), the
threshold is element-count `>=` at all three sites, the vendored
base64 is strict where it matters, `materialize_tensor_bin` can't
cycle (JSON tree) and its double-visit is a no-op, `tensor.load`'s
payload-as-wire preserves dtype/endianness/nonfinite bits verbatim
with correctly-ordered bounds checks, and `wire.py` mirrors the Rust
dtype/length validation. The engine-eqsat arr-leaf refusal is
verified against `term_bridge.rs`'s actual `to_kernel` behavior, the
pratt multi-statement change matches `eval_block` semantics exactly,
and the whisper alias positions check out against both
`_build_static_inputs` and the forward's `past` slicing. The one real
gap is the already-named `hnl run` egress — tracked, not silent.

**Verdict: PASS with one LOW + two standing MEDIUMs** — all new
committed code audits clean; P18-N1 is a documented follow-up rather
than a defect, and both MEDIUMs are repo/pin-state issues carried
from pass 17 (kit-discovery still unlanded; no v0.1.18 tag exists to
pin the ADR-0179 graphopt half).
