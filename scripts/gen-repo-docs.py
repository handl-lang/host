#!/usr/bin/env python3
"""Write per-repo `project/references.md` and `project/tasks/0128-extraction.md`.

Derived from project/docs-split.yaml — nothing is hand-maintained per repo:

  references.md            the canonical ADRs this repo depends on (they stay
                           in `platform`) + the child ADRs it owns
  tasks/0128-extraction.md that repo's ADR-0128 phase, expanded

Run from the platform repo: `python3 scripts/gen-repo-docs.py [--check]`.
"""

from __future__ import annotations

import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
ORG = ROOT.parent
MANIFEST = ROOT / "project" / "docs-split.yaml"
PLATFORM = "https://github.com/handl-lang/host/blob/main/project/adrs"

PHASE = {
    "python": 1, "c": 2, "stdlib": 3, "otlp": 4, "web": 5, "native": 6,
    "compile-dot": 7, "compile-term": 7, "compile-sexp": 7, "compile-handl": 7, "host-cli": 8, "host-lsp": 8,
    "meta": 9, "ffi": 10, "lang": 11, "kit-canonical": 11, "docs": 12,
}

REF_BY_REPO = {
    "lang": ["0044", "0052", "0055", "0074", "0081", "0096", "0106", "0123", "0129"],
    "kit-canonical": ["0039", "0044", "0092", "0099", "0129"],
    "stdlib": ["0013", "0077", "0083", "0093", "0129"],
    "web": ["0003", "0107", "0114", "0116", "0117", "0121", "0129"],
    "python": ["0107", "0129"],
    "c": ["0107", "0129"],
    "native": ["0114", "0129"],
    "host-cli": ["0029", "0085", "0090", "0129"],
    "host-lsp": ["0100", "0124", "0129"],
    "meta": ["0050", "0085", "0103", "0105", "0125", "0129"],
    "ffi": ["0083", "0107", "0129"],
    "platform": ["0128", "0129"],
}


def emit(path: Path, text: str, force: bool) -> str:
    """Write `path` unless it already exists and `force` is off.

    Existing files may be hand-edited (a curated `references.md`, ticked-off
    extraction steps) — regenerating is opt-in via `--force`.
    """
    if path.exists() and not force:
        return f"kept   {path}"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return f"wrote  {path}"


def main(argv: list[str] | None = None) -> int:
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", action="append", default=None,
                    help="only this repo (repeatable); default: all with a project worktree")
    ap.add_argument("--force", action="store_true",
                    help="overwrite existing files (default: never clobber hand-edited docs)")
    args = ap.parse_args(argv)

    data = yaml.safe_load(MANIFEST.read_text())
    repos = data.get("repos") or {}
    splits = data.get("splits") or []

    # repo -> [child target dict]
    owned: dict[str, list[dict]] = {}
    # repo -> [parent slug]
    parents: dict[str, set[str]] = {}
    for s in splits:
        parent = s["parent"].split("/", 1)[1]
        for t in s.get("targets") or []:
            repo = t["repo"]
            if repo not in repos:
                continue
            parents.setdefault(repo, set()).add(parent)
            if t.get("number"):
                owned.setdefault(repo, []).append(t)

    results: list[str] = []
    for repo in repos:
        if repo == "platform":
            continue
        if args.repo and repo not in args.repo:
            continue
        proj = ORG / repo / "project"
        if not proj.is_dir():
            print(f"skip  {repo} (no project worktree)")
            continue

        refs = REF_BY_REPO.get(repo, ["0129"])
        lines = [
            "# Canonical ADR references",
            "",
            "ADRs that define ports, the kernel, or composition stay in",
            "`handl-lang/host/project/adrs/`. They are referenced here, never",
            "copied (ADR-0129 §2/§7).",
            "",
            "| ADR | url |",
            "|---|---|",
        ]
        for num in refs:
            lines.append(f"| {num} | {PLATFORM}/{num}-*.md |")

        if owned.get(repo):
            lines += ["", "## ADRs this repo owns", "", "| # | title | file |", "|---|---|---|"]
            for t in owned[repo]:
                lines.append(
                    f"| {t['number']} | {t['title']} | "
                    f"`adrs/{t['number']}-{t['slug']}.md` |"
                )
        if parents.get(repo):
            lines += ["", "## Split parents (stay in platform)", ""]
            for p in sorted(parents[repo]):
                lines.append(f"- `{p}`")
        results.append(emit(proj / "references.md", "\n".join(lines) + "\n", args.force))

        phase = PHASE.get(repo)
        if phase is None:
            continue
        (proj / "tasks").mkdir(exist_ok=True)
        task = [
            f"# Task 0128 — `{repo}/` extraction (ADR-0128 Phase {phase})",
            "",
            f"Expand ADR-0128 Phase {phase} for this repo. The docs are already on",
            "this `project` branch; the code lands with the extraction.",
            "",
            "## Docs owned here",
            "",
        ]
        for t in owned.get(repo, []):
            task.append(f"- `adrs/{t['number']}-{t['slug']}.md` — {t['title']}")
        for p in sorted(parents.get(repo, [])):
            task.append(f"- split parent `{p}` (canonical half stays in `platform`)")
        task += [
            "",
            "## Extraction",
            "",
            f"- [ ] Follow ADR-0128 Phase {phase}: create the repo's workspace,",
            "      move the crates/packages named there, wire the git dep + `[patch]`.",
            "- [ ] Keep `main` a clean build tree (`/project/` ignored).",
            "- [ ] Flip the split children from `Proposed` to `Accepted` when the",
            "      code actually moves.",
            "",
        ]
        results.append(emit(proj / "tasks" / "0128-extraction.md", "\n".join(task), args.force))
    print("\n".join(results))
    print(f"\n{sum(1 for r in results if r.startswith('wrote'))} written, "
          f"{sum(1 for r in results if r.startswith('kept'))} kept (use --force to overwrite)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
