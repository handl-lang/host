# Audit pass 22 — Step E codec consolidation verified fleet-wide

Scope: kit-discovery v0.1.3/v0.1.4, kit-onnx v0.1.4, kit-torch v0.1.4,
source-onnx @ eb87b63, eqsat v0.1.12, stale-pin sweep, worktree hygiene.

## Verdicts

1. kit-discovery v0.1.4 (8491090) hnl_codec shim — **PASS**. 63-line
   re-export of mlopt_core.term_json; HnlCodecError aliased; private
   codec gone; tests assert canonical emit + legacy decode-only.
2. kit-discovery v0.1.3 (88d93db) pins — **PASS**. eqsat v0.1.12 ×3,
   kit-torch v0.1.4, carriers v0.1.5, gamekit v0.1.14, discoverykit
   v0.1.23; no override-dependencies block.
3. kit-onnx v0.1.4 + kit-torch v0.1.4 .term repoint — **PASS**. Zero
   mlopt_core.ir in either src tree.
4. source-onnx @ eb87b63 canonical emit — **PASS**. "Native" source,
   attrs channel, indexed dag dims; no Foreign/lit/nested-dim emitted.
5. eqsat v0.1.12 term repr B — **PASS**. term.py + term_json.py exist;
   ir.py is a compat shim. RECORD_VERSION=3 lives in mlopt/stage.py
   (attribution corrected; value unchanged).
6. Stale-pin sweep — **PASS** except known graphopt in-flight.
   mlopt+kernelopt pin kit-discovery@v0.1.3 (one tag behind v0.1.4 —
   fold into next repin).
7. Worktree hygiene — stepe worktree was merged/clean; parent removed
   it + deleted branch.

## Findings
- P22-N1 LOW: mlopt/kernelopt kit-discovery@v0.1.3 → bump to v0.1.4 next repin.
- P22-N2 LOW→RESOLVED: annotated tags confirmed via rev-list:
  kit-discovery v0.1.3=88d93db v0.1.4=8491090; kit-onnx v0.1.4=b2c8ff6;
  kit-torch v0.1.4=9cabf1af. All correct.
- P22-N3 INFO: RECORD_VERSION attribution (mlopt stage.py:135).
- P22-N4 INFO→RESOLVED: /home/luis/tmp/kit-discovery-stepe removed.

**Verdict: PASS.**
