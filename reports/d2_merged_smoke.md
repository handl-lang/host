# D-2 merged-tree product smoke — graphopt-d2-dryrun @ `00a5d17`

Worktree: `/home/luis/tmp/graphopt-d2-dryrun` (detached HEAD `00a5d17`,
mlopt `adae555` content merged into `packages/mlopt`). Venv pre-synced
with `--extra hf --extra kokoro`. No pushes, no `gh`, no HF downloads —
all refs exercised are local/synthetic. Closes the "not run" caveat in
`d2_dryrun_refresh.md` §"Not run".

## Results

| # | Item | Verdict | Notes |
|---|------|---------|-------|
| 1 | `.venv` + `packages/mlopt` populated | **PASS** | `.venv/bin/mlopt` + `.venv/bin/python` present; `packages/mlopt/{src,tests,arch-*.hnl}` populated; `git status` clean, HEAD = `00a5d17` |
| 2 | `mlopt models` — adapter catalog | **PASS** | all 5 adapters `ready`: `arch`, `hnl`, `kokoro`, `onnx-rust`, `whisper` (took ~4 min — torch/transformers import cost, not a hang) |
| 3 | `mlopt doctor` on a download-free ref | **PASS** | `hnl` adapter probe on a tiny `.hnl` (parse + leaf census only, no weight load) |
| 4 | `hnl mlopt models` via hnl-ext-mlopt | **PASS** | `hnl` not on PATH; used `host-cli/target/debug/hnl` + prebuilt `kit-prelude/target/debug/libhnl_ext_mlopt.so` (no cargo build needed) |
| 5 | `mlopt stage` JSON round-trip | **PASS** | full `lift → search → verify` chain over the `test_stage.py` tiny fixture; `verified.passed: true`, `max_abs: 0.0` |

## Exact commands + outputs

### 1. Environment

```sh
cd /home/luis/tmp/graphopt-d2-dryrun
ls .venv/bin/mlopt .venv/bin/python   # both exist
git rev-parse HEAD                    # 00a5d17c196bbfa0376fcfe87ad4ece34d766403
git status --porcelain                # clean
```

### 2. `mlopt models`

```sh
.venv/bin/mlopt models
```

```
arch       ready
           mlopt.arch/v1 architecture spec — source-onnx Rust assembler ...
hnl        ready
           .hnl Handl-surface source — hnl_to_term lift, tensor.* op surface ...
kokoro     ready
           Kokoro TTS (hexgrad/Kokoro-82M family) ...
onnx-rust  ready
           .onnx ONNX source — source-onnx Rust lifter ...
whisper    ready
           HF Whisper (all sizes ...) ...
```

### 3. `mlopt doctor`

Fixture (same as `packages/mlopt/tests/test_stage.py::tiny_hnl`, written to
`/tmp/d2-smoke/tiny.hnl` to keep the worktree clean):

```hnl
--- tiny test program
from "host-tensor" import tensor
do {
  w := tensor.reshape(tensor.from_list([1, 2, 3, 4, 5, 6]), [2, 3])
  tensor.matmul(x, w)
}
```

```sh
.venv/bin/mlopt doctor /tmp/d2-smoke/tiny.hnl
```

```
ref:       /tmp/d2-smoke/tiny.hnl
adapter:   hnl
status:    available
probe.parse: ok
probe.vars: ['x']
probe.params: ['w']
probe.note: leaf values bind at load: feed=<json> for real data, else seeded synthetic ...
```

Probe is parse + census only — no weights touched. `whisper:`/`kokoro:`/
`arch:`/`onnx-rust:` refs were deliberately not doctor'd: every one needs
HF weights or a sibling Rust checkout's safetensors, all of which risk a
download (the `arch-*.safetensors` were deleted in the refresh).

**Honest-decline observation (not a failure):** `mlopt doctor
packages/mlopt-kit/hnl/conv_bn.hnl` reports `probe.parse: declined —
HnlImportError: unexpected character ':' at offset 78`. That file is a
handl-dialect program (`export name :=`, `---` doc comments), not the
`.hnl` codec surface the `hnl` adapter lifts — the decline is the
product's honest-verdict contract working as designed, but worth noting
that a `.hnl`-suffixed handl file is *claimed* by the adapter and then
declines at parse.

### 4. `hnl mlopt models`

```sh
HANDL_EXT_DIR=/home/luis/GitHub/handl-lang/kit-prelude/target/debug \
MLOPT_PYTHON=/home/luis/tmp/graphopt-d2-dryrun/.venv/bin/python \
/home/luis/GitHub/handl-lang/host-cli/target/debug/hnl mlopt models
```

Identical 5-adapter `ready` listing as item 2 — the cdylib shim
(`libhnl_ext_mlopt.so`, already built in kit-prelude `target/debug`)
forwards to `python -m mlopt` in the worktree venv. No cargo build was
required.

### 5. Stage-protocol smoke (lift → search → verify)

```sh
echo '{"model":"hnl:/tmp/d2-smoke/tiny.hnl","adapter_opts":{"leaves":{"x":[4,2]}}}' \
  | .venv/bin/python -m mlopt stage lift    > lift.json   # ok:true, stage:lift, adapter:hnl
echo '<lift.json>'  | .venv/bin/python -m mlopt stage search > search.json  # ok:true, engine:eqsat, 341 laws, search_s:0.033
echo '<search.json>'| .venv/bin/python -m mlopt stage verify > verify.json  # ok:true, verified.passed:true, max_abs:0.0, verify_s:0.069
```

Notable: search resolved the eqsat binary at
`/home/luis/GitHub/handl-lang/engine-eqsat/target/release/eqsat-run`
(prebuilt, sibling checkout) — the merged member finds it without
configuration. Leaf values re-materialised per stage as
`leaf_source: synthetic` per the protocol contract.

## Verdict

**PASS on all five items.** The D-2 merged product works end-to-end:
catalog discovery, doctor probe, the hnl extension seam, and the full
stage protocol (lift→search→verify with a real eqsat run and a passing
verify gate). No downloads were triggered; the only SKIP-shaped surface
(weighted adapters: whisper/kokoro/arch/onnx-rust `load` paths) was
avoided by design, not blocked. This closes the last caveat on the D-2
GO verdict from `d2_dryrun_refresh.md`.
