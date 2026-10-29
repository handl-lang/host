#!/usr/bin/env python3
"""Check a `project/` tree against the ADR-0162 layout.

The top-level set is closed: one artifact type per question. Published
documentation does not live here — it belongs to the org `docs` repo.

Report-only by default; `--strict` fails on anything classified as an error.

Usage:
    python3 scripts/check-project-layout.py
    python3 scripts/check-project-layout.py --repo ../graphopt
    python3 scripts/check-project-layout.py --strict
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent

DIRS = {"adrs", "design", "rfcs", "tasks", "notes", "reports", "retros"}
FILES = {"TODO.md", "BACKLOG.md", "references.md", "parked.yaml",
         "docs-split.yaml", "README.md"}
# Present in the wild today; each has a named home.
DRIFT = {
    "docs": "published docs belong to the org `docs` repo (docs/site/…)",
    "plans": "use `tasks/` for work, `design/` for mechanism",
    "assets": "keep assets beside the document, or in docs/public/",
    "structure.md": "use README.md (the map) — ADR-0162",
    "GOALS.md": "use BACKLOG.md",
    "research_lab_notebook.md": "use `notes/`",
}
DATED = {"reports", "retros", "notes"}
NUM = re.compile(r"^(\d+[a-z]?)-")
REFS = re.compile(r"^[-*]?\s*Refs\s*:", re.M)
LOCAL_HDR = re.compile(r"^##\s+Local entries\s*$(.*?)(?=^##\s|\Z)", re.M | re.S)
SKIP = {".git", ".gitignore", ".keep"}
FRONT = re.compile(r"\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n", re.S)


def frontmatter(path: Path) -> dict:
    """ADR-0163 header of a `project/` file ({} when absent or malformed)."""
    m = FRONT.match(path.read_text(encoding="utf-8", errors="replace"))
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1)) or {}
    except yaml.YAMLError:
        return {}
    return data if isinstance(data, dict) else {}


def local_entries(proj: Path) -> dict[str, str]:
    """`## Local entries` in project/README.md declares repo-local additions."""
    readme = proj / "README.md"
    if not readme.is_file():
        return {}
    m = LOCAL_HDR.search(readme.read_text(encoding="utf-8", errors="replace"))
    if not m:
        return {}
    out: dict[str, str] = {}
    for line in m.group(1).splitlines():
        hit = re.match(r"^\s*[-*]\s*`?([\w.\-/]+?)`?\s+[—–-]\s+(.+)$", line)
        if hit:
            out[hit.group(1).rstrip("/")] = hit.group(2).strip()
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=Path, default=ROOT, help="repo root (default: this repo)")
    ap.add_argument("--strict", action="store_true", help="exit 1 on any error")
    ap.add_argument("--strict-all", action="store_true",
                    help="exit 1 on drift warnings too (aspirational)")
    args = ap.parse_args(argv)

    proj = (args.repo / "project").resolve()
    if not proj.is_dir():
        # the org `project` repo *is* a project tree (no nested project/)
        proj = args.repo.resolve()
    print(f"project tree: {proj}")
    if not any((proj / d).is_dir() for d in ("adrs", "design", "tasks", "notes")):
        print(f"  warning: {proj} does not look like a project tree", file=sys.stderr)

    errors: list[str] = []
    warns: list[str] = []
    adr_numbers: set[str] = set()
    local = local_entries(proj)

    if (proj / "adrs").is_dir():
        adr_numbers = {NUM.match(p.name).group(1)
                       for p in (proj / "adrs").glob("*.md") if NUM.match(p.name)}

    for entry in sorted(proj.iterdir()):
        name = entry.name
        if name in SKIP:
            continue
        if entry.is_dir():
            if name in local:
                continue                      # declared repo-local entry
            if name not in DIRS:
                hint = DRIFT.get(name, "not one of the closed set (ADR-0162)")
                bucket = errors if name == "docs" else warns
                bucket.append(f"{name}/ — {hint}")
                continue
            if name == "adrs":
                for sub in sorted(entry.iterdir()):
                    if sub.is_dir() and sub.name != "archived":
                        errors.append(f"adrs/{sub.name}/ — ADRs live flat in adrs/ (archived/ only)")
            if name == "design":
                for f in sorted(entry.glob("*.md")):
                    m = NUM.match(f.name)
                    body = f.read_text(encoding="utf-8", errors="replace")[:2000]
                    has_refs = bool(REFS.search(body)) or bool(
                        frontmatter(f).get("refs"))
                    if not m and not has_refs:
                        warns.append(f"design/{f.name} — prefix the ADR number, "
                                     "or add `refs:` to the frontmatter (ADR-0163)")
                    elif m and m.group(1) not in adr_numbers:
                        warns.append(f"design/{f.name} — no ADR-{m.group(1)} in adrs/")
            if name in DATED:
                for f in sorted(entry.glob("*.md")):
                    if not frontmatter(f).get("date"):
                        warns.append(f"{name}/{f.name} — dated artifact: "
                                     "frontmatter needs `date:` (ADR-0163)")
        else:
            if name not in FILES and name not in local:
                hint = DRIFT.get(name, "not one of the closed set (ADR-0162)")
                warns.append(f"{name} — {hint}")

    # per-domain trees: any nested dir carrying its own adrs/
    for nested in sorted(proj.rglob("adrs")):
        if nested.parent == proj:
            continue
        area = nested.parent.relative_to(proj).parts[0]
        if area in local:
            continue                      # staging area for a repo not yet created
        errors.append(f"{nested.relative_to(proj)}/ — one project/ per repo; "
                      "ADRs live in the owning repo's project/adrs")

    print(f"  entries: {', '.join(sorted(p.name for p in proj.iterdir() if p.name not in SKIP))}\n")

    if warns:
        print(f"Unclassified / drift ({len(warns)}):")
        for w in warns[:12]:
            print(f"  {w}")
        if len(warns) > 12:
            print(f"  … {len(warns) - 12} more")
        print()
    if errors:
        print(f"Errors ({len(errors)}):")
        for e in errors:
            print(f"  {e}")
        print()

    if not warns and not errors:
        print("Layout matches ADR-0162.")
    # errors = the tree is wrong; warnings = drift inventory (ADR-0162 §4
    # grandfathers files that predate the rule).
    bad = bool(errors) or (args.strict_all and bool(warns))
    print("FAIL" if bad else "OK")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
