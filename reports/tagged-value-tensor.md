# Report: `TaggedValue::Tensor` — the opaque-tensor kernel variant

Date: this session. Commits: kernel `b57e922`, platform `224813ca`.

## What landed

- **`TaggedValue::Tensor(Rc<TensorValue>)`** in `handl-value` (kernel
  `5ed78ee..b57e922`). `TensorValue { shape: Vec<usize>, data: Rc<Vec<f64>> }`
  is a dense row-major `f64` tensor — `data.len() == product(shape)` is
  enforced by `TensorValue::new` (checked product, overflow rejected).
- **Wire form is the host-tensor encoding**
  `{"__tensor__": {"shape": [...], "data": [...], "dtype": "f64"}}`,
  matching `host-tensor/src/ops.rs`'s `encode_tensor`/`decode_tensor`
  exactly: `dtype` optional on decode, `"f64"` the only accepted value.
  Malformed payloads fall back to a plain object (the lenient
  `__bytes__`/`__sym__` convention), so a valid `__tensor__` object never
  silently changes rep either direction.
- **A tensor is a value, not a handle**: structural equality over
  shape + data (elementwise `f64` `==`, matching `Float` — `0.0 == -0.0`,
  `NaN` unequal to itself). `Hash` agrees: elements hash as `f64` bits
  with `-0.0` normalized to `+0.0`.
- **Platform**: the only non-exhaustive `TaggedValue` match in the
  workspace was `type_tag` (`plugin-engine-exec/src/native_ops.rs`);
  `typeof` now reports `"tensor"`, matching the lowercase scalar-tag
  convention (`"bytes"`, `"resource"`, `"sym"`).

## Findings

- **`value_types.yaml` is inert spec** — no code consumes it; the real
  runtime rep is the `TaggedValue` enum (`kernel/crates/handl-value`).
  The `host-tensor` doc comment anticipated this: "promoting one in
  `value_types.yaml` later changes no wire shape" — confirmed, the wire
  form is unchanged.
- **The VM stays generic**: engines pass `Tensor` through host fns; no
  `is_tensor` dispatch was added anywhere in an engine. `field_access`
  on a tensor is a type error (same as `Sym`/`Resource`); `arrow_free`
  treats it as substitutable literal data (it round-trips JSON
  losslessly); `check_portable` accepts it (self-contained data, unlike
  `__resource__`).
- **`ValueAbi`** (`plugin-compile`) does not know `__tensor__`: it flows
  through as a generic `Object` and round-trips — the carrier contract.
  The reserved-key list in `ValueAbi::check` (`__pair__`, `__either__`,
  `__arrow__`, `__bytes__`, `__resource__`) was left unchanged —
  `__sym__` is already absent from it, and adding `__tensor__` would
  reject legitimate tensor payloads crossing the ABI as objects.

## Deferred

- `host-tensor` could adopt `TaggedValue::as_tensor`/`tensor()` instead of
  its local `Tensor` codec — not done (out of scope; the wire form is
  identical either way).
- A `Tensor` entry in `lang`'s `value_types.yaml` would keep the spec in
  sync; skipped because the file is inert.
- `Eq` caveat inherited: `f64` `==` is non-reflexive for `NaN`, so
  `TaggedValue`'s `impl Eq` remains technically loose (pre-existing
  `Float` behavior; `Tensor` follows the same convention).
