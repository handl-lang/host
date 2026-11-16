# llama-1L verify-run — end-to-end verdict

**Verdict: PASS** — `max_abs = 6.128e-14` over 4,096,000 values
(shape `[1, 128, 32000]`, f64), 0 mismatches.

- Tolerance applied (replicating `mlopt.cli._verify_run_tolerance`,
  f64 reference + F32 sidecar payload): `atol = rtol = 6.104e-05`
  (`512·eps(f32)` widened bound).
- The result is also **under the strict f64-vs-f64 bound `1e-9`** by
  ~4 orders of magnitude — the earlier `4.53e-7` discrepancy is
  **resolved**.  It was the f32-creator noise fixed by the
  `mlopt_torch` `dtype_ctx` change (graphopt `3e724a8`, hand-copied
  into the mlopt venv, byte-identical).

## What ran

Split verify-run (same decomposition the earlier llama agent used —
`mlopt export --verify-run` re-does the torch reference in-process;
on this 11 GB host that path is what earlyoom kept killing):

1. `/home/luis/tmp/llama1_verify_prog.hnl` — the exact stdin program
   `KitRunner` would eval (emitted earlier by
   `/home/luis/tmp/llama1_split_verify.py`; header already stripped,
   `tensor.load("llama1_e2e.safetensors", …)` leaves intact).
2. `/home/luis/tmp/llama1_ref.npy` + `llama1_ref_meta.json` — the f64
   torch reference (`label=f64`, shape `[1, 128, 32000]`).
3. Host side: file-arg mode (stdin mode flaked previously):

   ```sh
   cd /home/luis/tmp
   TENSOR_LOAD_DIR=/home/luis/tmp \
     /home/luis/GitHub/handl-lang/graphopt/target/debug/hnl-run \
     llama1_verify_prog.hnl > llama1_hnl_out_v3.json
   ```

   Binary: `graphopt/target/debug/hnl-run` built 2026-11-16 09:08 —
   postdates host-tensor `tensor.load` streaming (`c488bad`).
   nice 19 / ionice idle, `rssmon.sh` polling `/proc` VmRSS 1 Hz.

4. Compare: `llama1_hnl_out_v3.json` `__tensor__.data` vs
   `llama1_ref.npy`, `max|a−b|` and the `atol + rtol·|b|` mismatch
   count from `cli.py:647-668`.

## Memory / infra notes

- First attempt (09:27–09:31) was **killed by earlyoom** —
  `journalctl`: `earlyoom … sending SIGTERM to process 1744291
  "hnl-run": VmRSS 3842 MiB` at mem-avail 9.85 % while a coverage
  pytest run (~1 GB) plus concurrent rustc/vitepress jobs from other
  sessions competed.  Infra-blocked, not a code failure; rssmon
  caught a 5,433,156 kB (≈5.2 GiB) sample before the kill.
- Retry at 09:31 with ~6.8 GB available: **completed in ~4 min,
  exit 0**, `peak_rss_kb = 5,252,228` (≈5.0 GiB).  The streaming
  `tensor.load` does reduce the steady-state footprint (observed
  RSS fell to ~1.5 GB during the compute phase after weight load),
  but the load+eval peak still grazes this box's earlyoom line
  (`-m 10 -s 15`) whenever anything else is resident.

## Artifacts

- `/home/luis/tmp/llama1_hnl_out_v3.json` — host output (78 MB)
- `/home/luis/tmp/llama1_v3.rss` — `peak_rss_kb=5252228 exit=0`
- `/home/luis/tmp/llama1_v3.err` — empty (clean stderr)
- First (killed) attempt: `llama1_hnl_out_v2.json` (empty),
  `llama1_v2.rss`, journal entry above.
