#!/usr/bin/env python3
"""project/docs-split.yaml tooling (ADR-0129 Phase 2).

Modes:
  check                    verify every file under project/ is assigned once
  paths <repo>             print `project/<path>` lines for `--paths-from-file`
  paths-all <dir>          write <dir>/paths-<repo>.txt for every non-empty repo

The path lists feed `git filter-repo --paths-from-file ... --path-rename project/:`
(see scripts/split-project-docs.sh).
"""

from __future__ import annotations

import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
MANIFEST = PROJECT / "docs-split.yaml"
SELF = "docs-split.yaml"

SUBDIR = {
    "root": "",
    "adrs": "adrs",
    "adrs_archived": "adrs/archived",
    "tasks": "tasks",
    "notes": "notes",
    "reports": "reports",
    "rfcs": "rfcs",
    "design": "design",
    "examples": "examples",
    "assets": "assets",
}


def load() -> dict:
    return yaml.safe_load(MANIFEST.read_text())


def repo_paths(data: dict, repo: str) -> list[str]:
    sections = (data.get("repos") or {}).get(repo, {}) or {}
    out = []
    for key, names in sections.items():
        if key not in SUBDIR:
            raise SystemExit(f"unknown section '{key}' in repo '{repo}'")
        for name in names or []:
            sub = SUBDIR[key]
            out.append(f"project/{sub}/{name}" if sub else f"project/{name}")
    return sorted(out)


def check(data: dict) -> int:
    assigned: dict[str, list[str]] = {}
    for repo in (data.get("repos") or {}):
        for rel in repo_paths(data, repo):
            assigned.setdefault(rel.removeprefix("project/"), []).append(repo)

    actual = {
        str(p.relative_to(PROJECT))
        for p in PROJECT.rglob("*")
        if p.is_file() and p.name != SELF
    }

    unmapped = sorted(actual - set(assigned))
    phantom = sorted(set(assigned) - actual)
    dupes = sorted(k for k, v in assigned.items() if len(v) > 1)

    for rel in unmapped:
        print(f"UNMAPPED   {rel}")
    for rel in phantom:
        print(f"PHANTOM    {rel}  (claimed by {', '.join(assigned[rel])})")
    for rel in dupes:
        print(f"DUPLICATE  {rel}  ({', '.join(assigned[rel])})")

    ok = not (unmapped or phantom or dupes)
    print(
        f"\n{len(actual)} files, {len(assigned)} assigned, "
        f"{len(data.get('splits') or [])} splits -> {'OK' if ok else 'FAIL'}"
    )
    return 0 if ok else 1


def main(argv: list[str]) -> int:
    data = load()
    if not argv or argv[0] == "check":
        return check(data)

    if argv[0] == "paths":
        if len(argv) != 2:
            raise SystemExit("usage: docs_split.py paths <repo>")
        print("\n".join(repo_paths(data, argv[1])))
        return 0

    if argv[0] == "paths-all":
        if len(argv) != 2:
            raise SystemExit("usage: docs_split.py paths-all <dir>")
        outdir = Path(argv[1])
        outdir.mkdir(parents=True, exist_ok=True)
        for repo in (data.get("repos") or {}):
            paths = repo_paths(data, repo)
            if not paths:
                continue
            (outdir / f"paths-{repo}.txt").write_text("\n".join(paths) + "\n")
            print(f"{outdir}/paths-{repo}.txt  ({len(paths)})")
        return 0

    raise SystemExit(__doc__)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
