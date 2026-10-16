# `split/*` scratch branches — retired (ADR-0129 4.3)

The 18 `split/*` branches were orphan scratch branches (no merge-base with
`main`) produced while extracting crates/packages into the target repos. They
are deleted; their tips are recorded here so any of them is recoverable:

```bash
git branch split/<name> <sha>      # restore
git show <sha>                     # inspect
```

Content status: the extracted repos (`lang`, `kit-canonical`) were created
from the `crates-*` pair, and every `handl-packages/*` branch corresponds to a
package that still exists under `handl-packages/` in `platform` (or in the
repo that now owns it, per `docs-split.yaml`). Nothing here is unique.

| branch | tip | commits |
|---|---|---|
| `split/crates-handl-kit-data-canonical` | `99322ffe` | 177 |
| `split/crates-kit-canonical` | `c625740f` | 211 |
| `split/handl-packages-comb` | `a62b76be` | 8 |
| `split/handl-packages-command` | `53ca1fdf` | 4 |
| `split/handl-packages-event` | `c3737e37` | 5 |
| `split/handl-packages-ffi` | `8144eadf` | 9 |
| `split/handl-packages-noise-c` | `aded73ef` | 1 |
| `split/handl-packages-noise-stub` | `540b1b8a` | 1 |
| `split/handl-packages-os-c` | `b6ccab46` | 6 |
| `split/handl-packages-os-stub` | `f7285f77` | 4 |
| `split/handl-packages-perlin` | `aa3213bd` | 2 |
| `split/handl-packages-query` | `660422e8` | 4 |
| `split/handl-packages-rand-stub` | `102d57d8` | 10 |
| `split/handl-packages-simplex` | `b7cd9ab7` | 2 |
| `split/handl-packages-test` | `fa1a5964` | 11 |
| `split/handl-packages-type-handl` | `b847bf4e` | 7 |
| `split/handl-packages-uuid-c` | `5d758a07` | 3 |
| `split/handl-packages-uuid-stub` | `d3afe128` | 1 |

Deleted 2026-10-15, after all 12 doc-bearing target repos had a verified
`project` branch (`scripts/docs-mount.sh --status`).
